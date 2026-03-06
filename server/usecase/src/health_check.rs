use domain::repository::health_check_repository::HealthCheckRepository;

pub struct HealthCheckResult {
    pub db: bool,
    pub meilisearch: bool,
    pub rabbitmq: bool,
    pub discord: bool,
}

impl HealthCheckResult {
    pub fn all_ok(&self) -> bool {
        self.db && self.meilisearch && self.rabbitmq && self.discord
    }
}

pub struct HealthCheckUseCase<'a> {
    pub repository: &'a (dyn HealthCheckRepository + Send + Sync),
}

impl HealthCheckUseCase<'_> {
    pub async fn check(&self) -> HealthCheckResult {
        let (db, meilisearch, rabbitmq, discord) = tokio::join!(
            self.repository.ping_db(),
            self.repository.ping_meilisearch(),
            self.repository.is_rabbitmq_connected(),
            self.repository.is_discord_connected(),
        );

        HealthCheckResult {
            db,
            meilisearch,
            rabbitmq,
            discord,
        }
    }
}
