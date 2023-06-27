use async_trait::async_trait;
use migration::MigratorTrait;
use sea_orm::{Database, DatabaseConnection, DatabaseTransaction, TransactionTrait};

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
}

#[async_trait]
impl DatabaseComponents for ConnectionPool {
    type ConcreteFormDatabase = Self;
    type ConcreteHealthCheckDatabase = Self;
    type TransactionAcrossComponents = DatabaseTransaction;

    async fn begin_transaction(&self) -> anyhow::Result<Self::TransactionAcrossComponents> {
        Ok(self.pool.begin().await?)
    }

    fn form(&self) -> &Self::ConcreteFormDatabase {
        self
    }

    fn health_check(&self) -> &Self::ConcreteHealthCheckDatabase {
        self
    }
}
