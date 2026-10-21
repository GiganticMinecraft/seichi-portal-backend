use std::{
    fmt::Debug,
    future::Future,
    pin::Pin,
    str::FromStr,
    sync::{Arc, RwLock},
    time::Duration,
};

use async_trait::async_trait;
use opentelemetry::{KeyValue, global, metrics::ObservableGauge};
use redis::{
    Client, RedisError, RedisResult,
    aio::{ConnectionManager, ConnectionManagerConfig},
};
use sqlx::{
    ConnectOptions, Connection, MySql,
    mysql::{MySqlConnectOptions, MySqlPoolOptions},
};

use crate::database::{
    components::DatabaseComponents,
    config::{MEILISEARCH, MYSQL, MeiliSearch, MySQL, REDIS, Redis},
};

pub type DatabaseTransaction = sqlx::Transaction<'static, MySql>;

/// セッションの読み書きなど、リクエスト処理中に行う Valkey 操作 1 回あたりの上限。
/// 再接続待ちとやり直しもこの時間に含めるため、Valkey が応答しない間もリクエストは詰まらずにエラーで返る。
const VALKEY_OPERATION_TIMEOUT: Duration = Duration::from_secs(1);
/// 接続の確立とコマンドの応答それぞれの上限。応答の時間切れで接続を作り直してやり直す余地を
/// [`VALKEY_OPERATION_TIMEOUT`] の中に残すため半分にする (クラスタ内なら張り直しは数 ms で終わる)。
const VALKEY_ATTEMPT_TIMEOUT: Duration = Duration::from_millis(500);
/// ヘルスチェックの PING の上限。readiness probe を素早く返すため、通常の操作より短くする。
const VALKEY_PING_TIMEOUT: Duration = Duration::from_millis(250);

/// プロセスで共有する Valkey 接続。
///
/// `ConnectionManager` は接続が切れたこと (RST や EOF) を検知すれば自分で張り直すが、応答が返らないだけの
/// 時間切れでは張り直さない。相手が RST を返さずに消えた half-open の接続だと、OS が TCP を諦めるまで
/// 全操作が時間切れになり続けるため、コマンドの応答が [`VALKEY_ATTEMPT_TIMEOUT`] 内に返らなかったときは
/// こちらで作り直す。切断の検知による張り直しは `ConnectionManager` に任せる。
#[derive(Clone, Debug)]
pub(crate) struct ValkeyConnection(Arc<ValkeyConnectionInner>);

#[derive(Debug)]
struct ValkeyConnectionInner {
    client: Client,
    config: ConnectionManagerConfig,
    /// (世代, 接続)。同時に失敗した複数の操作が何度も作り直さないよう、世代が一致したときだけ差し替える。
    current: RwLock<(u64, ConnectionManager)>,
}

impl ValkeyConnection {
    async fn connect() -> Self {
        let Redis { host, port } = &*REDIS;
        let client = Client::open(format!("redis://{host}:{port}/"))
            .unwrap_or_else(|error| panic!("Invalid Valkey connection URL: {error}"));
        let config = ConnectionManagerConfig::new()
            .set_connection_timeout(Some(VALKEY_ATTEMPT_TIMEOUT))
            .set_response_timeout(Some(VALKEY_ATTEMPT_TIMEOUT));

        let manager = ConnectionManager::new_with_config(client.clone(), config.clone())
            .await
            .unwrap_or_else(|error| panic!("Cannot establish Valkey connection: {error}"));

        Self(Arc::new(ValkeyConnectionInner {
            client,
            config,
            current: RwLock::new((0, manager)),
        }))
    }

    fn current(&self) -> (u64, ConnectionManager) {
        self.0
            .current
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// 世代 `generation` の接続を捨てて作り直す。実行中の操作は古い接続の clone を持っているので途中で切れない。
    fn reset(&self, generation: u64) {
        let mut current = self
            .0
            .current
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if current.0 != generation {
            return;
        }
        match ConnectionManager::new_lazy_with_config(self.0.client.clone(), self.0.config.clone())
        {
            Ok(manager) => *current = (generation + 1, manager),
            Err(error) => tracing::warn!(%error, "failed to recreate Valkey connection"),
        }
    }

    /// `command` を共有接続で実行し、全体を `timeout` で打ち切る。時間切れは I/O の TimedOut として返す。
    ///
    /// 1 回目が失敗したときだけ 1 回やり直すため、やり直しても安全な冪等なコマンドだけを渡すこと。
    /// - 応答の時間切れ (`is_timeout()`): 接続を作り直し、新しい接続でやり直す
    /// - 接続が切れた (`is_unrecoverable_error()`): `ConnectionManager` が張り直しを始めているので同じ接続でやり直す
    ///
    /// `timeout` による打ち切り (再接続待ちを含む) では作り直さない。Valkey が落ちている間に作り直すと、
    /// 古い `ConnectionManager` の再接続と並んで再接続が積み重なるため。
    async fn run<T, F, Fut>(&self, timeout: Duration, command: F) -> RedisResult<T>
    where
        F: Fn(ConnectionManager) -> Fut,
        Fut: Future<Output = RedisResult<T>>,
    {
        let operation = async {
            let (generation, connection) = self.current();
            match command(connection.clone()).await {
                Err(error) if error.is_timeout() => {
                    self.reset(generation);
                    let (generation, connection) = self.current();
                    let result = command(connection).await;
                    if matches!(&result, Err(error) if error.is_timeout()) {
                        self.reset(generation);
                    }
                    result
                }
                Err(error) if error.is_unrecoverable_error() => command(connection).await,
                result => result,
            }
        };

        tokio::time::timeout(timeout, operation)
            .await
            .unwrap_or_else(|_| {
                Err(RedisError::from(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Valkey operation timed out",
                )))
            })
    }
}

#[derive(Clone, Debug)]
pub struct ConnectionPool {
    pub(crate) rdb_pool: sqlx::MySqlPool,
    pub(crate) minecraft_bans_pool: sqlx::MySqlPool,
    pub(crate) meilisearch_client: meilisearch_sdk::client::Client,
    /// 起動時に 1 つだけ作り、clone して使い回す。内部は 1 本の多重化接続。
    pub(crate) valkey_connection: ValkeyConnection,
}

impl ConnectionPool {
    fn database_url() -> String {
        let MySQL {
            user,
            password,
            host,
            port,
            database,
            ..
        } = &*MYSQL;

        format!("mysql://{user}:{password}@{host}:{port}/{database}")
    }

    /// SQL 文のログ出力は止める。
    ///
    /// stdout のログでは元々 `sqlx::query` を出していない (bind 値を含みうるため)。そのうえ sqlx は
    /// クエリごとに `log::log_enabled!` / `tracing::enabled!` で出力要否を問い合わせ、tracing-subscriber
    /// 0.3 の per-layer filter ではその問い合わせの判定が残って直後のログが捨てられることがあるため
    /// (`entrypoint::telemetry::LogSpanFilter` を参照)、問い合わせ自体をしないようにする。
    /// クエリの所要時間は database 層のクライアントスパンで Tempo に残る。
    fn connect_options(url: &str) -> MySqlConnectOptions {
        MySqlConnectOptions::from_str(url)
            .unwrap_or_else(|_| panic!("Invalid MySQL connection URL."))
            .disable_statement_logging()
    }

    pub async fn new() -> Self {
        let database_url = Self::database_url();
        let MeiliSearch { host, api_key } = &*MEILISEARCH;

        let rdb_pool = MySqlPoolOptions::new()
            .connect_with(Self::connect_options(&database_url))
            .await
            .unwrap_or_else(|_| panic!("Cannot establish portal database connection."));
        let minecraft_bans_database_url = std::env::var("MINECRAFT_BANS_DATABASE_URL")
            .unwrap_or_else(|_| panic!("MINECRAFT_BANS_DATABASE_URL is not set."));
        let minecraft_bans_pool = MySqlPoolOptions::new()
            .connect_with(Self::connect_options(&minecraft_bans_database_url))
            .await
            .unwrap_or_else(|_| panic!("Cannot establish Minecraft bans database connection."));

        let valkey_connection = ValkeyConnection::connect().await;

        Self {
            rdb_pool,
            minecraft_bans_pool,
            meilisearch_client: meilisearch_sdk::client::Client::new(host, api_key.to_owned())
                .unwrap_or_else(|_| panic!("Cannot establish connect to MeiliSearch.")),
            valkey_connection,
        }
    }

    /// DB コネクションプールの使用状況をメトリクスとして登録する。
    ///
    /// global meter provider を設定した後に呼ぶこと。返り値を drop するとコールバックが外れうるため、
    /// プロセス終了まで保持する。属性名は OpenTelemetry の database client semantic conventions に合わせる。
    pub fn register_pool_metrics(&self) -> Vec<ObservableGauge<u64>> {
        let pools = [
            ("portal", self.rdb_pool.clone()),
            ("minecraft_bans", self.minecraft_bans_pool.clone()),
        ];
        let meter = global::meter("seichi-portal-backend");
        let count_pools = pools.clone();

        vec![
            meter
                .u64_observable_gauge("db.client.connection.count")
                .with_description("状態ごとのコネクション数")
                .with_unit("{connection}")
                .with_callback(move |observer| {
                    count_pools.iter().for_each(|(name, pool)| {
                        let idle = pool.num_idle() as u64;
                        let used = u64::from(pool.size()).saturating_sub(idle);
                        [("idle", idle), ("used", used)]
                            .into_iter()
                            .for_each(|(state, count)| {
                                observer.observe(
                                    count,
                                    &[
                                        KeyValue::new("db.client.connection.pool.name", *name),
                                        KeyValue::new("db.client.connection.state", state),
                                    ],
                                );
                            });
                    });
                })
                .build(),
            meter
                .u64_observable_gauge("db.client.connection.max")
                .with_description("プールが開けるコネクション数の上限")
                .with_unit("{connection}")
                .with_callback(move |observer| {
                    pools.iter().for_each(|(name, pool)| {
                        observer.observe(
                            u64::from(pool.options().get_max_connections()),
                            &[KeyValue::new("db.client.connection.pool.name", *name)],
                        );
                    });
                })
                .build(),
        ]
    }

    pub async fn ping_db(&self) -> bool {
        let (portal_db, minecraft_bans_db) = tokio::join!(
            Self::ping_pool(&self.rdb_pool),
            Self::ping_pool(&self.minecraft_bans_pool),
        );
        portal_db && minecraft_bans_db
    }

    async fn ping_pool(pool: &sqlx::MySqlPool) -> bool {
        let Ok(mut connection) = pool.acquire().await else {
            return false;
        };
        connection.ping().await.is_ok()
    }

    /// 共有の Valkey 接続でコマンドを実行する。全体を [`VALKEY_OPERATION_TIMEOUT`] で打ち切る。
    /// 失敗時に 1 回だけやり直すため、SET / GET / DEL のような冪等なコマンドだけを渡すこと
    /// (INCR や LPUSH などは二重に適用されうる)。
    pub(crate) async fn idempotent_valkey_command<T, F, Fut>(
        &self,
        command: F,
    ) -> Result<T, InfraError>
    where
        F: Fn(ConnectionManager) -> Fut,
        Fut: Future<Output = RedisResult<T>>,
    {
        self.valkey_connection
            .run(VALKEY_OPERATION_TIMEOUT, command)
            .await
            .map_err(InfraError::from)
    }

    /// 共有の Valkey 接続に PING を送り、[`VALKEY_PING_TIMEOUT`] 内に PONG が返るかを見る。
    ///
    /// PING 自体は別タスクで通常の操作と同じ上限まで走らせる。probe の上限 (250ms) で打ち切ると、
    /// 応答の時間切れ (500ms) による接続の作り直しが起きず、half-open の接続のまま readiness が
    /// 落ち続けて (トラフィックが来ないので) 戻らなくなるため。
    pub async fn ping_valkey(&self) -> bool {
        let connection = self.valkey_connection.clone();
        let ping = tokio::spawn(async move {
            connection
                .run(VALKEY_OPERATION_TIMEOUT, |mut connection| async move {
                    redis::cmd("PING")
                        .query_async::<String>(&mut connection)
                        .await
                })
                .await
        });
        let response = tokio::time::timeout(VALKEY_PING_TIMEOUT, ping).await;
        matches!(response, Ok(Ok(Ok(response))) if response == "PONG")
    }

    pub async fn ping_meilisearch(&self) -> bool {
        self.meilisearch_client
            .health()
            .await
            .map(|h| h.status == "available")
            .unwrap_or(false)
    }

    pub async fn migrate(&self) -> anyhow::Result<()> {
        migration::MIGRATOR.run(&self.rdb_pool).await?;
        Ok(())
    }

    // Alloy (k8s-monitoring) の set_semconv_span_name は DB の client span の名前を
    // db.operation.name から付け直す。付けないと span 名が "mariadb" だけになり区別できない
    #[tracing::instrument(
        skip_all,
        fields(otel.kind = "client", db.system = "mariadb", db.operation.name = "read_only_transaction")
    )]
    pub async fn read_only_transaction<F, T, E>(&self, callback: F) -> Result<T, InfraError>
    where
        F: for<'c> FnOnce(
                &'c mut DatabaseTransaction,
            ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'c>>
            + Send,
        T: Send,
        E: Into<InfraError> + Send,
    {
        let mut transaction = self
            .rdb_pool
            .begin_with("START TRANSACTION READ ONLY")
            .await
            .map_err(|error| InfraError::DatabaseTransaction {
                cause: error.to_string(),
            })?;

        let result = callback(&mut transaction).await;
        match result {
            Ok(value) => {
                transaction.commit().await?;
                Ok(value)
            }
            Err(error) => {
                let infra_error = error.into();
                let _ = transaction.rollback().await;
                Err(infra_error)
            }
        }
    }

    #[tracing::instrument(
        skip_all,
        fields(otel.kind = "client", db.system = "mariadb", db.operation.name = "read_write_transaction")
    )]
    pub async fn read_write_transaction<F, T, E>(&self, callback: F) -> Result<T, E>
    where
        F: for<'c> FnOnce(
                &'c mut DatabaseTransaction,
            ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'c>>
            + Send,
        T: Send,
        E: From<InfraError> + Send,
    {
        let mut transaction = self
            .rdb_pool
            .begin_with("START TRANSACTION READ WRITE")
            .await
            .map_err(|error| InfraError::DatabaseTransaction {
                cause: error.to_string(),
            })?;

        let result = callback(&mut transaction).await;
        match result {
            Ok(value) => {
                transaction.commit().await.map_err(InfraError::from)?;
                Ok(value)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }
}

#[async_trait]
impl DatabaseComponents for ConnectionPool {
    type ConcreteDiscordAPI = Self;
    type ConcreteFormAnswerDatabase = Self;
    type ConcreteFormAnswerRelationDatabase = Self;
    type ConcreteFormAnswerLabelDatabase = Self;
    type ConcreteFormCommentDatabase = Self;
    type ConcreteFormCommentAttachmentDatabase = Self;
    type ConcreteFormDatabase = Self;
    type ConcreteFormLabelDatabase = Self;
    type ConcreteFormMessageDatabase = Self;
    type ConcreteFormSubmissionRestrictionDatabase = Self;
    type ConcreteNotificationDatabase = Self;
    type ConcreteSearchDatabase = Self;
    type ConcreteMinecraftBanDatabase = Self;
    type ConcreteUserDatabase = Self;
    fn form(&self) -> &Self::ConcreteFormDatabase {
        self
    }

    fn form_answer(&self) -> &Self::ConcreteFormAnswerDatabase {
        self
    }

    fn form_answer_relation(&self) -> &Self::ConcreteFormAnswerRelationDatabase {
        self
    }

    fn form_answer_label(&self) -> &Self::ConcreteFormAnswerLabelDatabase {
        self
    }

    fn form_message(&self) -> &Self::ConcreteFormMessageDatabase {
        self
    }

    fn form_comment(&self) -> &Self::ConcreteFormCommentDatabase {
        self
    }

    fn form_comment_attachment(&self) -> &Self::ConcreteFormCommentAttachmentDatabase {
        self
    }

    fn form_label(&self) -> &Self::ConcreteFormLabelDatabase {
        self
    }

    fn form_submission_restriction(&self) -> &Self::ConcreteFormSubmissionRestrictionDatabase {
        self
    }

    fn user(&self) -> &Self::ConcreteUserDatabase {
        self
    }

    fn discord_api(&self) -> &Self::ConcreteDiscordAPI {
        self
    }

    fn search(&self) -> &Self::ConcreteSearchDatabase {
        self
    }

    fn notification(&self) -> &Self::ConcreteNotificationDatabase {
        self
    }

    fn minecraft_ban(&self) -> &Self::ConcreteMinecraftBanDatabase {
        self
    }
}

use errors::infra::InfraError;
