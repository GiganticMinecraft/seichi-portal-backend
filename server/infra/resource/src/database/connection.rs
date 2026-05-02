use std::{fmt::Debug, future::Future, pin::Pin};

use async_trait::async_trait;
use redis::Client;
use sqlx::{MySql, mysql::MySqlPoolOptions};

use crate::database::{
    components::DatabaseComponents,
    config::{MEILISEARCH, MYSQL, MeiliSearch, MySQL, REDIS, Redis},
};

pub type DatabaseTransaction = sqlx::Transaction<'static, MySql>;

#[derive(Clone, Debug)]
pub struct ConnectionPool {
    pub(crate) rdb_pool: sqlx::MySqlPool,
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

        Self {
            rdb_pool: MySqlPoolOptions::new()
                .connect(&database_url)
                .await
                .unwrap_or_else(|_| panic!("Cannot establish connect to {database_url}.")),
            meilisearch_client: meilisearch_sdk::client::Client::new(host, api_key.to_owned())
                .unwrap_or_else(|_| panic!("Cannot establish connect to MeiliSearch.")),
        }
    }

    pub async fn ping_db(&self) -> bool {
        sqlx::query("SELECT 1")
            .execute(&self.rdb_pool)
            .await
            .is_ok()
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

    pub async fn read_write_transaction<F, T, E>(&self, callback: F) -> Result<T, InfraError>
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
            .begin_with("START TRANSACTION READ WRITE")
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
}

#[async_trait]
impl DatabaseComponents for ConnectionPool {
    type ConcreteDiscordAPI = Self;
    type ConcreteFormAnswerDatabase = Self;
    type ConcreteFormAnswerLabelDatabase = Self;
    type ConcreteFormCommentDatabase = Self;
    type ConcreteFormDatabase = Self;
    type ConcreteFormLabelDatabase = Self;
    type ConcreteFormMessageDatabase = Self;
    type ConcreteFormQuestionDatabase = Self;
    type ConcreteNotificationDatabase = Self;
    type ConcreteSearchDatabase = Self;
    type ConcreteUserDatabase = Self;
    type TransactionAcrossComponents = DatabaseTransaction;

    async fn begin_transaction(&self) -> anyhow::Result<Self::TransactionAcrossComponents> {
        Ok(self
            .rdb_pool
            .begin_with("START TRANSACTION READ WRITE")
            .await?)
    }

    fn form(&self) -> &Self::ConcreteFormDatabase {
        self
    }

    fn form_answer(&self) -> &Self::ConcreteFormAnswerDatabase {
        self
    }

    fn form_answer_label(&self) -> &Self::ConcreteFormAnswerLabelDatabase {
        self
    }

    fn form_question(&self) -> &Self::ConcreteFormQuestionDatabase {
        self
    }

    fn form_message(&self) -> &Self::ConcreteFormMessageDatabase {
        self
    }

    fn form_comment(&self) -> &Self::ConcreteFormCommentDatabase {
        self
    }

    fn form_label(&self) -> &Self::ConcreteFormLabelDatabase {
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
}

pub async fn redis_connection() -> Client {
    let Redis { host, port } = &*REDIS;

    let redis_url = format!("redis://{host}:{port}/");

    let client_result = Client::open(redis_url);

    client_result.unwrap_or_else(|_| panic!("Cannot connect to Valkey."))
}

use errors::infra::InfraError;
