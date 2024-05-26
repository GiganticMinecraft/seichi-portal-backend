pub mod domain;
pub mod infra;
pub mod presentation;
pub mod usecase;

use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum Error {
    #[error(transparent)]
    Domain {
        #[from]
        source: domain::DomainError,
    },
    #[error(transparent)]
    Infra {
        #[from]
        source: infra::InfraError,
    },
    #[error(transparent)]
    Presentation {
        #[from]
        source: presentation::PresentationError,
    },
    #[error(transparent)]
    UseCase {
        #[from]
        source: usecase::UseCaseError,
    },
}
