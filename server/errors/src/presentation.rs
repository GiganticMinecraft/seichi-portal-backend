use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum PresentationError {
    #[error("Form not found.")]
    FormNotFound,
}
