use async_trait::async_trait;
use domain::repository::health_check_repository::HealthCheckRepository;

use crate::{
    database::components::{DatabaseComponents, HealthCheckDataBase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> HealthCheckRepository for Repository<Client> {
    async fn health_check(&self) -> bool {
        self.client.health_check().health_check().await
    }
}
