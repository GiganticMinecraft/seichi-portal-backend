use domain::repository::health_check_repository::{
    ComponentHealth, ComponentRequirement, HealthCheckRepository,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HealthStatus {
    Ok,
    Degraded,
    Error,
}

impl HealthStatus {
    pub fn is_ready(self) -> bool {
        self != Self::Error
    }
}

pub struct HealthCheckResult {
    pub components: Vec<ComponentHealth>,
}

impl HealthCheckResult {
    pub fn status(&self) -> HealthStatus {
        if self.components.iter().any(|component| {
            component.requirement == ComponentRequirement::Required && !component.healthy
        }) {
            HealthStatus::Error
        } else if self.components.iter().any(|component| !component.healthy) {
            HealthStatus::Degraded
        } else {
            HealthStatus::Ok
        }
    }

    pub fn is_ready(&self) -> bool {
        self.status().is_ready()
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

#[cfg(test)]
mod tests {
    use super::{HealthCheckUseCase, HealthStatus};
    use async_trait::async_trait;
    use domain::repository::health_check_repository::{
        ComponentHealth, ComponentRequirement, HealthCheckRepository,
    };

    struct FakeHealthCheckRepository {
        components: Vec<(ComponentRequirement, bool)>,
    }

    #[async_trait]
    impl HealthCheckRepository for FakeHealthCheckRepository {
        async fn check_components(&self) -> Vec<ComponentHealth> {
            self.components
                .iter()
                .enumerate()
                .map(|(index, (requirement, healthy))| ComponentHealth {
                    name: format!("component-{index}"),
                    requirement: *requirement,
                    healthy: *healthy,
                })
                .collect()
        }
    }

    async fn status_of(components: Vec<(ComponentRequirement, bool)>) -> HealthStatus {
        HealthCheckUseCase {
            repository: &FakeHealthCheckRepository { components },
        }
        .check()
        .await
        .status()
    }

    #[tokio::test]
    async fn all_healthy_components_are_ready() {
        assert_eq!(
            status_of(vec![
                (ComponentRequirement::Required, true),
                (ComponentRequirement::Required, true),
                (ComponentRequirement::Optional, true),
            ])
            .await,
            HealthStatus::Ok
        );
    }

    #[tokio::test]
    async fn unhealthy_mariadb_makes_health_check_error() {
        assert_eq!(
            status_of(vec![
                (ComponentRequirement::Required, false),
                (ComponentRequirement::Required, true),
                (ComponentRequirement::Optional, true),
            ])
            .await,
            HealthStatus::Error
        );
    }

    #[tokio::test]
    async fn unhealthy_valkey_makes_health_check_error() {
        assert_eq!(
            status_of(vec![
                (ComponentRequirement::Required, true),
                (ComponentRequirement::Required, false),
                (ComponentRequirement::Optional, true),
            ])
            .await,
            HealthStatus::Error
        );
    }

    #[tokio::test]
    async fn unhealthy_optional_component_degrades_but_remains_ready() {
        assert_eq!(
            status_of(vec![
                (ComponentRequirement::Required, true),
                (ComponentRequirement::Required, true),
                (ComponentRequirement::Optional, false),
            ])
            .await,
            HealthStatus::Degraded
        );
    }

    #[tokio::test]
    async fn required_failure_takes_precedence_over_optional_degradation() {
        let result = HealthCheckUseCase {
            repository: &FakeHealthCheckRepository {
                components: vec![
                    (ComponentRequirement::Required, false),
                    (ComponentRequirement::Optional, false),
                ],
            },
        }
        .check()
        .await;

        assert_eq!(result.status(), HealthStatus::Error);
        assert!(!result.is_ready());
    }

    #[tokio::test]
    async fn degraded_status_is_ready() {
        let result = HealthCheckUseCase {
            repository: &FakeHealthCheckRepository {
                components: vec![
                    (ComponentRequirement::Required, true),
                    (ComponentRequirement::Optional, false),
                ],
            },
        }
        .check()
        .await;

        assert_eq!(result.status(), HealthStatus::Degraded);
        assert!(result.is_ready());
    }
}
