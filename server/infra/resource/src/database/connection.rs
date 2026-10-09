use std::{fmt::Debug, future::Future, pin::Pin, str::FromStr, time::Duration};

use async_trait::async_trait;
use opentelemetry::{KeyValue, global, metrics::ObservableGauge};
use redis::Client;
use sqlx::{
    ConnectOptions, Connection, MySql,
    mysql::{MySqlConnectOptions, MySqlPoolOptions},
};

use crate::database::{
    components::DatabaseComponents,
    config::{MEILISEARCH, MYSQL, MeiliSearch, MySQL, REDIS, Redis},
};

pub type DatabaseTransaction = sqlx::Transaction<'static, MySql>;

const VALKEY_OPERATION_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Clone, Debug)]
pub struct ConnectionPool {
    pub(crate) rdb_pool: sqlx::MySqlPool,
    pub(crate) minecraft_bans_pool: sqlx::MySqlPool,
    pub(crate) meilisearch_client: meilisearch_sdk::client::Client,
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

        Self {
            rdb_pool,
            minecraft_bans_pool,
            meilisearch_client: meilisearch_sdk::client::Client::new(host, api_key.to_owned())
                .unwrap_or_else(|_| panic!("Cannot establish connect to MeiliSearch.")),
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

pub async fn redis_connection() -> Client {
    let Redis { host, port } = &*REDIS;

    let redis_url = format!("redis://{host}:{port}/");

    let client_result = Client::open(redis_url);

    client_result.unwrap_or_else(|_| panic!("Cannot connect to Valkey."))
}

pub async fn ping_valkey() -> bool {
    let (Ok(host), Ok(port)) = (std::env::var("REDIS_HOST"), std::env::var("REDIS_PORT")) else {
        return false;
    };
    let Ok(client) = Client::open(format!("redis://{host}:{port}/")) else {
        return false;
    };
    let connection = tokio::time::timeout(
        VALKEY_OPERATION_TIMEOUT,
        client.get_multiplexed_async_connection(),
    )
    .await;
    let Ok(Ok(mut connection)) = connection else {
        return false;
    };

    let response = tokio::time::timeout(
        VALKEY_OPERATION_TIMEOUT,
        redis::cmd("PING").query_async::<String>(&mut connection),
    )
    .await;
    matches!(response, Ok(Ok(response)) if response == "PONG")
}

use errors::infra::InfraError;
