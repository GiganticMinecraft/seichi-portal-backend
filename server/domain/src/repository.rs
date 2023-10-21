pub mod form_repository;
pub mod user_repository;

pub trait Repositories: Send + Sync {
    type ConcreteFormRepository: form_repository::FormRepository;
    type ConcreteUserRepository: user_repository::UserRepository;

    fn form_repository(&self) -> &Self::ConcreteFormRepository;
    fn user_repository(&self) -> &Self::ConcreteUserRepository;
}
