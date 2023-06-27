use domain::repository::health_check_repository::HealthCheckRepository;

pub struct HealthCheckUseCase<'a, HealthCheckRepo: HealthCheckRepository> {
    pub repository: &'a HealthCheckRepo,
}

impl<R: HealthCheckRepository> HealthCheckUseCase<'_, R> {
    pub async fn health_check(&self) -> bool {
        self.repository.health_check().await
    }
}
