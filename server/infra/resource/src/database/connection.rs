use std::{fmt::Debug, future::Future, pin::Pin, time::Duration};

use async_trait::async_trait;
use redis::Client;
use sqlx::{Connection, MySql, mysql::MySqlPoolOptions};

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

    pub async fn new() -> Self {
        let database_url = Self::database_url();
        let MeiliSearch { host, api_key } = &*MEILISEARCH;

        let rdb_pool = MySqlPoolOptions::new()
            .connect(&database_url)
            .await
            .unwrap_or_else(|_| panic!("Cannot establish portal database connection."));
        let minecraft_bans_database_url = std::env::var("MINECRAFT_BANS_DATABASE_URL")
            .unwrap_or_else(|_| panic!("MINECRAFT_BANS_DATABASE_URL is not set."));
        let minecraft_bans_pool = MySqlPoolOptions::new()
            .connect(&minecraft_bans_database_url)
            .await
            .unwrap_or_else(|_| panic!("Cannot establish Minecraft bans database connection."));

        Self {
            rdb_pool,
            minecraft_bans_pool,
            meilisearch_client: meilisearch_sdk::client::Client::new(host, api_key.to_owned())
                .unwrap_or_else(|_| panic!("Cannot establish connect to MeiliSearch.")),
        }
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

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "mariadb"))]
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

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "mariadb"))]
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

/// Redmine importer が必要とする Portal DB だけを開く専用接続です。
///
/// 通常の [`ConnectionPool`] は Valkey・Minecraft bans DB・Meilisearch も初期化するため、
/// 一回限りの移行 Job が通常 API の依存関係や副作用を読み込まないよう分離します。
#[derive(Clone, Debug)]
pub struct RedmineImportConnectionPool {
    pub(crate) rdb_pool: sqlx::MySqlPool,
}

impl RedmineImportConnectionPool {
    pub async fn new() -> anyhow::Result<Self> {
        let rdb_pool = MySqlPoolOptions::new()
            .connect(&ConnectionPool::database_url())
            .await?;

        Ok(Self { rdb_pool })
    }

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "mariadb"))]
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

    #[tracing::instrument(skip_all, fields(otel.kind = "client", db.system = "mariadb"))]
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
