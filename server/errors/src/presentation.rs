use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum_extra::typed_header::TypedHeaderRejection;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum PresentationError {
    #[error("Json Rejection: {}", .cause)]
    JsonRejection { cause: String },
    #[error("Path Rejection: {}", .cause)]
    PathRejection { cause: String },
    #[error("Query Rejection: {}", .cause)]
    QueryRejection { cause: String },
    #[error("Typed Header Rejection: {}", .cause)]
    TypedHeaderRejection { cause: String },
}

impl From<JsonRejection> for PresentationError {
    fn from(value: JsonRejection) -> Self {
        PresentationError::JsonRejection {
            cause: value.body_text(),
        }
    }
}

impl From<PathRejection> for PresentationError {
    fn from(value: PathRejection) -> Self {
        PresentationError::PathRejection {
            cause: value.body_text(),
        }
    }
}

impl From<QueryRejection> for PresentationError {
    fn from(value: QueryRejection) -> Self {
        PresentationError::QueryRejection {
            cause: value.to_string(),
        }
    }
}

impl From<TypedHeaderRejection> for PresentationError {
    fn from(value: TypedHeaderRejection) -> Self {
        PresentationError::TypedHeaderRejection {
            cause: value.to_string(),
        }
    }
}
