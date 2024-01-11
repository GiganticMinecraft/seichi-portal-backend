use async_trait::async_trait;
use itertools::Itertools;
use migration::MigratorTrait;
use regex::Regex;
use sea_orm::{
    AccessMode, ConnectionTrait, Database, DatabaseBackend, DatabaseConnection,
    DatabaseTransaction, DbErr, ExecResult, QueryResult, Statement, TransactionError,
    TransactionTrait, Value,
};
use std::future::Future;
use std::pin::Pin;

use crate::database::{
    components::DatabaseComponents,
    config::{MySQL, MYSQL},
};

#[derive(Clone, Debug)]
pub struct ConnectionPool {
    pub(crate) pool: DatabaseConnection,
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

        Self {
            pool: Database::connect(&database_url)
                .await
                .unwrap_or_else(|_| panic!("Cannot establish connect to {database_url}.")),
        }
    }

    pub async fn migrate(&self) -> anyhow::Result<()> {
        migration::Migrator::up(&self.pool, None).await?;

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
        Ok(self
            .pool
            .transaction_with_config(callback, None, Some(AccessMode::ReadOnly))
            .await?)
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
        Ok(self
            .pool
            .transaction_with_config(callback, None, Some(AccessMode::ReadWrite))
            .await?)
    }

    pub async fn query_all(&self, sql: &str) -> Result<Vec<QueryResult>, DbErr> {
        self.pool
            .query_all(Statement::from_string(DatabaseBackend::MySql, sql))
            .await
    }

    pub async fn query_all_and_values<I>(
        &self,
        sql: &str,
        values: I,
    ) -> Result<Vec<QueryResult>, DbErr>
    where
        I: IntoIterator<Item = Value>,
    {
        self.pool
            .query_all(Statement::from_sql_and_values(
                DatabaseBackend::MySql,
                sql,
                values,
            ))
            .await
    }

    pub async fn query_one_and_values<I>(
        &self,
        sql: &str,
        values: I,
    ) -> Result<Option<QueryResult>, DbErr>
    where
        I: IntoIterator<Item = Value>,
    {
        self.pool
            .query_one(Statement::from_sql_and_values(
                DatabaseBackend::MySql,
                sql,
                values,
            ))
            .await
    }

    pub async fn execute(&self, sql: &str) -> Result<ExecResult, DbErr> {
        self.pool
            .execute(Statement::from_string(DatabaseBackend::MySql, sql))
            .await
    }

    pub async fn execute_and_values<I>(&self, sql: &str, values: I) -> Result<ExecResult, DbErr>
    where
        I: IntoIterator<Item = Value>,
    {
        self.pool
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::MySql,
                sql,
                values,
            ))
            .await
    }

    pub async fn batch_insert<I>(&self, sql: &str, params: I) -> Result<Option<ExecResult>, DbErr>
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
                self.pool
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
}

#[async_trait]
impl DatabaseComponents for ConnectionPool {
    type ConcreteFormDatabase = Self;
    type ConcreteUserDatabase = Self;
    type TransactionAcrossComponents = DatabaseTransaction;

    async fn begin_transaction(&self) -> anyhow::Result<Self::TransactionAcrossComponents> {
        Ok(self.pool.begin().await?)
    }

    fn form(&self) -> &Self::ConcreteFormDatabase {
        self
    }

    fn user(&self) -> &Self::ConcreteUserDatabase {
        self
    }
}
