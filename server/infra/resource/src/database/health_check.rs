use async_trait::async_trait;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

use crate::database::{components::HealthCheckDataBase, connection::ConnectionPool};

#[async_trait]
impl HealthCheckDataBase for ConnectionPool {
    async fn health_check(&self) -> bool {
        self.pool
            .query_one(Statement::from_string(
                DatabaseBackend::MySql,
                "SELECT 1 FROM health_check WHERE false;".to_owned(),
            ))
            .await
            .is_ok()
    }
}
