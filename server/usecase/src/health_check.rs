use domain::repository::health_check_repository::{ComponentHealth, HealthCheckRepository};

pub struct HealthCheckResult {
    pub components: Vec<ComponentHealth>,
}

impl HealthCheckResult {
    pub fn all_ok(&self) -> bool {
        self.components.iter().all(|c| c.healthy)
    }
}

pub struct HealthCheckUseCase<'a, R: HealthCheckRepository + ?Sized> {
    pub repository: &'a R,
}

impl<R: HealthCheckRepository + ?Sized> HealthCheckUseCase<'_, R> {
    pub async fn check(&self) -> HealthCheckResult {
        let components = self.repository.check_components().await;
        HealthCheckResult { components }
    }
}
