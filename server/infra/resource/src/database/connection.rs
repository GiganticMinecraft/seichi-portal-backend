use std::{fmt::Debug, future::Future, pin::Pin};

use async_trait::async_trait;
use itertools::Itertools;
use migration::MigratorTrait;
use redis::Client;
use regex::Regex;
use sea_orm::{
    AccessMode, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection,
    DatabaseTransaction, DbErr, ExecResult, QueryResult, Statement, TransactionError,
    TransactionTrait, Value,
};

use crate::database::{
    components::DatabaseComponents,
    config::{MeiliSearch, MySQL, Redis, MEILISEARCH, MYSQL, REDIS},
};

#[derive(Clone, Debug)]
pub struct ConnectionPool {
    pub(crate) rdb_pool: DatabaseConnection,
    pub(crate) meilisearch_client: meilisearch_sdk::client::Client,
}

impl ConnectionPool {
    pub async fn new() -> Self {
        let MySQL {
            user,
            password,
            host,
            port,
            database,
            ..
        } = &*MYSQL;

        let database_url = format!("mysql://{user}:{password}@{host}:{port}/{database}");

        let MeiliSearch { host, api_key } = &*MEILISEARCH;

        Self {
            rdb_pool: Database::connect(&database_url)
                .await
                .unwrap_or_else(|_| panic!("Cannot establish connect to {database_url}.")),
            meilisearch_client: meilisearch_sdk::client::Client::new(host, api_key.to_owned())
                .unwrap_or_else(|_| panic!("Cannot establish connect to MeiliSearch.")),
        }
    }

    pub async fn migrate(&self) -> anyhow::Result<()> {
        migration::Migrator::up(&self.rdb_pool, None).await?;

        Ok(())
    }

    pub async fn read_only_transaction<F, T, E>(
        &self,
        callback: F,
    ) -> Result<T, TransactionError<E>>
    where
        F: for<'c> FnOnce(
                &'c DatabaseTransaction,
            ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'c>>
            + Send,
        T: Send,
        E: std::error::Error + Send,
    {
        self.rdb_pool
            .transaction_with_config(callback, None, Some(AccessMode::ReadOnly))
            .await
    }

    pub async fn read_write_transaction<F, T, E>(
        &self,
        callback: F,
    ) -> Result<T, TransactionError<E>>
    where
        F: for<'c> FnOnce(
                &'c DatabaseTransaction,
            ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'c>>
            + Send,
        T: Send,
        E: std::error::Error + Send,
    {
        self.rdb_pool
            .transaction_with_config(callback, None, Some(AccessMode::ReadWrite))
            .await
    }
}

#[async_trait]
impl DatabaseComponents for ConnectionPool {
    type ConcreteFormDatabase = Self;
    type ConcreteMessageDatabase = Self;
    type ConcreteSearchDatabase = Self;
    type ConcreteUserDatabase = Self;
    type TransactionAcrossComponents = DatabaseTransaction;

    async fn begin_transaction(&self) -> anyhow::Result<Self::TransactionAcrossComponents> {
        Ok(self.rdb_pool.begin().await?)
    }

    fn form(&self) -> &Self::ConcreteFormDatabase {
        self
    }

    fn user(&self) -> &Self::ConcreteUserDatabase {
        self
    }

    fn search(&self) -> &Self::ConcreteSearchDatabase {
        self
    }

    fn message(&self) -> &Self::ConcreteMessageDatabase {
        self
    }
}

pub async fn query_all(
    sql: &str,
    transaction: &DatabaseTransaction,
) -> Result<Vec<QueryResult>, DbErr> {
    transaction
        .query_all(Statement::from_string(DatabaseBackend::MySql, sql))
        .await
}

pub async fn query_all_and_values<I>(
    sql: &str,
    values: I,
    transaction: &DatabaseTransaction,
) -> Result<Vec<QueryResult>, DbErr>
where
    I: IntoIterator<Item = Value>,
{
    transaction
        .query_all(Statement::from_sql_and_values(
            DatabaseBackend::MySql,
            sql,
            values,
        ))
        .await
}

pub async fn query_one(
    sql: &str,
    transaction: &DatabaseTransaction,
) -> Result<Option<QueryResult>, DbErr> {
    transaction
        .query_one(Statement::from_string(DatabaseBackend::MySql, sql))
        .await
}

pub async fn query_one_and_values<I>(
    sql: &str,
    values: I,
    transaction: &DatabaseTransaction,
) -> Result<Option<QueryResult>, DbErr>
where
    I: IntoIterator<Item = Value>,
{
    transaction
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::MySql,
            sql,
            values,
        ))
        .await
}

pub async fn execute(sql: &str, transaction: &DatabaseTransaction) -> Result<ExecResult, DbErr> {
    transaction
        .execute(Statement::from_string(DatabaseBackend::MySql, sql))
        .await
}

pub async fn execute_and_values<I>(
    sql: &str,
    values: I,
    transaction: &DatabaseTransaction,
) -> Result<ExecResult, DbErr>
where
    I: IntoIterator<Item = Value>,
{
    transaction
        .execute(Statement::from_sql_and_values(
            DatabaseBackend::MySql,
            sql,
            values,
        ))
        .await
}

pub async fn batch_insert<I>(
    sql: &str,
    params: I,
    transaction: &DatabaseTransaction,
) -> Result<Option<ExecResult>, DbErr>
where
    I: IntoIterator<Item = Value>,
{
    let regex = Regex::new(r"\((\?,\s*)+\?\)").unwrap();
    let insert_part_opt = regex.find(sql);

    assert!(
        insert_part_opt.is_some(),
        "SQL insert params must be exists."
    );

    let params_vec = params.into_iter().collect::<Vec<_>>();

    if params_vec.is_empty() {
        Ok(None)
    } else {
        let insert_part = insert_part_opt.unwrap().as_str();

        Ok(Some(
            transaction
                .execute(Statement::from_sql_and_values(
                    DatabaseBackend::MySql,
                    sql.replace(
                        insert_part,
                        &vec![insert_part; params_vec.len() / insert_part.matches('?').count()]
                            .iter()
                            .join(", "),
                    ),
                    params_vec,
                ))
                .await?,
        ))
    }
}

pub async fn multiple_delete<I>(
    sql: &str,
    params: I,
    transaction: &DatabaseTransaction,
) -> Result<Option<ExecResult>, DbErr>
where
    I: IntoIterator<Item = Value>,
{
    let params_vec = params.into_iter().collect::<Vec<_>>();

    if params_vec.is_empty() {
        Ok(None)
    } else {
        Ok(Some(
            transaction
                .execute(Statement::from_sql_and_values(
                    DatabaseBackend::MySql,
                    sql.replace(
                        "(?)",
                        format!("({})", &vec!["?"; params_vec.len()].iter().join(", ")).as_str(),
                    ),
                    params_vec,
                ))
                .await?,
        ))
    }
}

pub async fn redis_connection() -> Client {
    let Redis { host, port } = &*REDIS;

    let redis_url = format!("redis://{host}:{port}/");

    let client_result = Client::open(redis_url);

    client_result.unwrap_or_else(|_| panic!("Cannot connect to Redis."))
}
