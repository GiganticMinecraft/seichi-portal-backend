use crate::database::components::DatabaseComponents;
use crate::database::config::{MySQL, MYSQL};
use async_trait::async_trait;
use sea_orm::{Database, DatabaseConnection, DatabaseTransaction, TransactionTrait};

#[derive(Clone)]
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
}

#[async_trait]
impl DatabaseComponents for ConnectionPool {
    type ConcreteFormDatabase = Self;
    type TransactionAcrossComponents = DatabaseTransaction;

    async fn begin_transaction(&self) -> anyhow::Result<Self::TransactionAcrossComponents> {
        Ok(self.pool.begin().await?)
    }

    fn form(&self) -> &Self::ConcreteFormDatabase {
        self
    }
}
