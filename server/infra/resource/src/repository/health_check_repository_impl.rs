use crate::database::components::{DatabaseComponents, HealthCheckDataBase};
use crate::repository::Repository;
use domain::repository::health_check_repository::HealthCheckRepository;

#[async_trait]
impl<Client: DatabaseComponents + 'static> HealthCheckRepository for Repository<Client> {
    async fn health_check(&self) -> bool {
        self.client.health_check().health_check()
    }
}
