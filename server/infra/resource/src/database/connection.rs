use async_trait::async_trait;
use migration::MigratorTrait;
use sea_orm::{
    ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, DatabaseTransaction, DbErr,
    ExecResult, QueryResult, Statement, TransactionTrait, Value,
};

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
