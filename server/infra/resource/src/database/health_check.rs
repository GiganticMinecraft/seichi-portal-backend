use crate::database::components::{FormDatabase, HealthCheckDataBase};
use crate::database::connection::ConnectionPool;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

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
