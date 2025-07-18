pub mod domain;
pub mod infra;
pub mod presentation;
pub mod usecase;
pub mod validation;

use crate::presentation::PresentationError;
use thiserror::Error;

pub trait ErrorExtra<T> {
    fn map_err_to_error(self) -> Result<T, Error>;
}

impl<T, E> ErrorExtra<T> for Result<T, E>
where
    E: Into<PresentationError>,
{
    fn map_err_to_error(self) -> Result<T, Error> {
        self.map_err(Into::into).map_err(Into::into)
    }
}

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
    #[error(transparent)]
    Presentation {
        #[from]
        source: presentation::PresentationError,
    },
}
