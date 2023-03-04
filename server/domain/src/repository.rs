pub mod form_repository;

pub trait Repositories: Send + Sync {
    type ConcreteFormRepository: form_repository::FormRepository;

    fn form_repository(&self) -> &Self::ConcreteFormRepository;
}
