pub mod domain;
pub mod infra;
pub mod usecase;
pub mod validation;

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
    UseCase {
        #[from]
        source: usecase::UseCaseError,
    },
    #[error(transparent)]
    Validation {
        #[from]
        source: validation::ValidationError,
    },
}
