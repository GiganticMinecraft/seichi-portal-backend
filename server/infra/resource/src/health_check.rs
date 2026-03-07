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
    async fn ping_db(&self) -> bool {
        self.db_conn.ping_db().await
    }

    async fn ping_meilisearch(&self) -> bool {
        self.db_conn.ping_meilisearch().await
    }

    async fn is_rabbitmq_connected(&self) -> bool {
        self.rabbitmq_conn.is_rabbitmq_connected()
    }

    async fn is_discord_connected(&self) -> bool {
        let runners = self.shard_manager.runners.lock().await;
        runners
            .values()
            .any(|r| r.stage == ConnectionStage::Connected)
    }
}
