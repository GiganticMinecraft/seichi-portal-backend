pub mod form_repository;
pub mod search_repository;
pub mod user_repository;

pub trait Repositories: Send + Sync {
    type ConcreteFormRepository: form_repository::FormRepository;
    type ConcreteUserRepository: user_repository::UserRepository;
    type ConcreteSearchRepository: search_repository::SearchRepository;

    fn form_repository(&self) -> &Self::ConcreteFormRepository;
    fn user_repository(&self) -> &Self::ConcreteUserRepository;
    fn search_repository(&self) -> &Self::ConcreteSearchRepository;
}
