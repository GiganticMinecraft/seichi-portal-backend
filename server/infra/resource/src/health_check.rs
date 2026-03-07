use std::sync::Arc;

use async_trait::async_trait;
use domain::repository::health_check_repository::HealthCheckRepository;
use serenity::gateway::ConnectionStage;

use crate::{database::connection::ConnectionPool, messaging::connection::MessagingConnectionPool};

pub struct HealthCheckRepositoryImpl {
    pub(crate) db_conn: Arc<ConnectionPool>,
    pub(crate) rabbitmq_conn: Arc<MessagingConnectionPool>,
    pub(crate) shard_manager: Arc<serenity::all::ShardManager>,
}

impl HealthCheckRepositoryImpl {
    pub fn new(
        db_conn: Arc<ConnectionPool>,
        rabbitmq_conn: Arc<MessagingConnectionPool>,
        shard_manager: Arc<serenity::all::ShardManager>,
    ) -> Self {
        Self {
            db_conn,
            rabbitmq_conn,
            shard_manager,
        }
    }
}

#[async_trait]
impl HealthCheckRepository for HealthCheckRepositoryImpl {
    async fn check_components(
        &self,
    ) -> Vec<domain::repository::health_check_repository::ComponentHealth> {
        use domain::repository::health_check_repository::ComponentHealth;

        let (db, meilisearch, rabbitmq, discord) = tokio::join!(
            self.db_conn.ping_db(),
            self.db_conn.ping_meilisearch(),
            async { self.rabbitmq_conn.is_rabbitmq_connected() },
            async {
                let runners = self.shard_manager.runners.lock().await;
                runners
                    .values()
                    .any(|r| r.stage == ConnectionStage::Connected)
            },
        );

        vec![
            ComponentHealth {
                name: "db".to_string(),
                healthy: db,
            },
            ComponentHealth {
                name: "meilisearch".to_string(),
                healthy: meilisearch,
            },
            ComponentHealth {
                name: "rabbitmq".to_string(),
                healthy: rabbitmq,
            },
            ComponentHealth {
                name: "discord".to_string(),
                healthy: discord,
            },
        ]
    }
}
