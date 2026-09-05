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
    #[error("Multipart Rejection: {}", .cause)]
    MultipartRejection { cause: String },
    #[error("Payload Too Large: {}", .cause)]
    PayloadTooLarge { cause: String },
}

impl From<JsonRejection> for PresentationError {
    fn from(value: JsonRejection) -> Self {
        PresentationError::JsonRejection {
            cause: value.body_text(),
        }
    }
}

impl From<axum::extract::multipart::MultipartRejection> for PresentationError {
    fn from(value: axum::extract::multipart::MultipartRejection) -> Self {
        PresentationError::MultipartRejection {
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

impl From<axum_extra::extract::QueryRejection> for PresentationError {
    fn from(value: axum_extra::extract::QueryRejection) -> Self {
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
