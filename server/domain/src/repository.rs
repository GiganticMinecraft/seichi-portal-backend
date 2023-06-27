pub mod form_repository;
pub mod health_check_repository;

pub trait Repositories: Send + Sync {
    type ConcreteFormRepository: form_repository::FormRepository;
    type ConcreteHealthCheckRepository: health_check_repository::HealthCheckRepository;

    fn form_repository(&self) -> &Self::ConcreteFormRepository;
    fn health_check_repository(&self) -> &Self::ConcreteHealthCheckRepository;
}
