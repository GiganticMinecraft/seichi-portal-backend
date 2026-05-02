use std::{fmt::Debug, future::Future, pin::Pin};

use async_trait::async_trait;
use migration::MigratorTrait;
use redis::Client;
use sea_orm::{Database, Value};
use sqlx::{
    MySql,
    mysql::{MySqlArguments, MySqlPool, MySqlPoolOptions, MySqlQueryResult, MySqlRow},
    query::Query,
};

use crate::database::{
    components::DatabaseComponents,
    config::{MEILISEARCH, MYSQL, MeiliSearch, MySQL, REDIS, Redis},
};

pub type DatabaseTransaction = sqlx::Transaction<'static, MySql>;
pub type DbErr = sqlx::Error;
pub type ExecResult = MySqlQueryResult;

#[derive(Clone, Debug)]
pub struct ConnectionPool {
    pub(crate) rdb_pool: MySqlPool,
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
        let migration_conn = Database::connect(Self::database_url()).await?;
        migration::Migrator::up(&migration_conn, None).await?;
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

fn bind_value<'q>(
    query: Query<'q, MySql, MySqlArguments>,
    value: Value,
) -> Result<Query<'q, MySql, MySqlArguments>, sqlx::Error> {
    match value {
        Value::Bool(value) => Ok(query.bind(value)),
        Value::TinyInt(value) => Ok(query.bind(value)),
        Value::SmallInt(value) => Ok(query.bind(value)),
        Value::Int(value) => Ok(query.bind(value)),
        Value::BigInt(value) => Ok(query.bind(value)),
        Value::TinyUnsigned(value) => Ok(query.bind(value)),
        Value::SmallUnsigned(value) => Ok(query.bind(value)),
        Value::Unsigned(value) => Ok(query.bind(value)),
        Value::BigUnsigned(value) => Ok(query.bind(value)),
        Value::Float(value) => Ok(query.bind(value)),
        Value::Double(value) => Ok(query.bind(value)),
        Value::String(value) => Ok(query.bind(value.map(|value| *value))),
        Value::Char(value) => Ok(query.bind(value.map(|value| value.to_string()))),
        Value::Bytes(value) => Ok(query.bind(value.map(|value| *value))),
        Value::ChronoDate(value) => Ok(query.bind(value.map(|value| *value))),
        Value::ChronoTime(value) => Ok(query.bind(value.map(|value| *value))),
        Value::ChronoDateTime(value) => Ok(query.bind(value.map(|value| *value))),
        Value::ChronoDateTimeUtc(value) => Ok(query.bind(value.map(|value| *value))),
        Value::ChronoDateTimeLocal(value) => {
            Ok(query.bind(value.map(|value| value.fixed_offset().naive_local())))
        }
        Value::ChronoDateTimeWithTimeZone(value) => {
            Ok(query.bind(value.map(|value| value.with_timezone(&chrono::Utc))))
        }
        Value::Uuid(value) => Ok(query.bind(value.map(|value| *value))),
        other => Err(sqlx::Error::Protocol(format!(
            "unsupported bind value for sqlx migration: {other:?}"
        ))),
    }
}

fn bind_values<'q, I>(
    sql: &'q str,
    values: I,
) -> Result<Query<'q, MySql, MySqlArguments>, sqlx::Error>
where
    I: IntoIterator<Item = Value>,
{
    values.into_iter().try_fold(sqlx::query(sql), bind_value)
}

pub async fn query_all(
    sql: &str,
    transaction: &mut DatabaseTransaction,
) -> Result<Vec<MySqlRow>, DbErr> {
    sqlx::query(sql).fetch_all(&mut **transaction).await
}

pub async fn query_all_and_values<I>(
    sql: &str,
    values: I,
    transaction: &mut DatabaseTransaction,
) -> Result<Vec<MySqlRow>, DbErr>
where
    I: IntoIterator<Item = Value>,
{
    bind_values(sql, values)?
        .fetch_all(&mut **transaction)
        .await
}

pub async fn query_one(
    sql: &str,
    transaction: &mut DatabaseTransaction,
) -> Result<Option<MySqlRow>, DbErr> {
    sqlx::query(sql).fetch_optional(&mut **transaction).await
}

pub async fn query_one_and_values<I>(
    sql: &str,
    values: I,
    transaction: &mut DatabaseTransaction,
) -> Result<Option<MySqlRow>, DbErr>
where
    I: IntoIterator<Item = Value>,
{
    bind_values(sql, values)?
        .fetch_optional(&mut **transaction)
        .await
}

pub async fn execute(
    sql: &str,
    transaction: &mut DatabaseTransaction,
) -> Result<ExecResult, DbErr> {
    sqlx::query(sql).execute(&mut **transaction).await
}

pub async fn execute_and_values<I>(
    sql: &str,
    values: I,
    transaction: &mut DatabaseTransaction,
) -> Result<ExecResult, DbErr>
where
    I: IntoIterator<Item = Value>,
{
    bind_values(sql, values)?.execute(&mut **transaction).await
}

pub async fn redis_connection() -> Client {
    let Redis { host, port } = &*REDIS;

    let redis_url = format!("redis://{host}:{port}/");

    let client_result = Client::open(redis_url);

    client_result.unwrap_or_else(|_| panic!("Cannot connect to Valkey."))
}

use errors::infra::InfraError;
