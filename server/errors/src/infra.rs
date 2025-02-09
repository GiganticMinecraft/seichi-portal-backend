use serde_json::Error;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error, PartialEq)]
pub enum InfraError {
    #[error("Database Error: {}", .source)]
    Database {
        #[from]
        source: sea_orm::error::DbErr,
    },
    #[error("Transaction Error: {}", .cause)]
    DatabaseTransaction { cause: String },
    #[error("Uuid Parse Error: {}", .source)]
    UuidParse {
        #[from]
        source: uuid::Error,
    },
    #[error("Form Not Found: id = {}", .id)]
    FormNotFound { id: Uuid },
    #[error("Answer Not Fount: id = {}", .id)]
    AnswerNotFount { id: i32 },
    #[error("Outgoing Error: {}", .cause)]
    Outgoing { cause: String },
    #[error("Enum Parse Error: source = {}", .source)]
    EnumParse {
        #[from]
        source: strum::ParseError,
    },
    #[error("Redis Error: {}", .source)]
    Redis {
        #[from]
        source: redis::RedisError,
    },
    #[error("Reqwest Error: {}", .cause)]
    Reqwest { cause: String },
    #[error("MeiliSearch Error: {}", .cause)]
    MeiliSearch { cause: String },
    #[error("SerdeJson Error: {}", .cause)]
    SerdeJson { cause: String },
    #[error("SerdeJson Error: {}", .cause)]
    SerenityError { cause: String },
}

impl<E> From<sea_orm::TransactionError<E>> for InfraError
where
    E: std::error::Error,
{
    fn from(value: sea_orm::TransactionError<E>) -> Self {
        InfraError::DatabaseTransaction {
            cause: value.to_string(),
        }
    }
}

impl From<meilisearch_sdk::errors::Error> for InfraError {
    fn from(value: meilisearch_sdk::errors::Error) -> Self {
        InfraError::MeiliSearch {
            cause: value.to_string(),
        }
    }
}

impl From<serde_json::Error> for InfraError {
    fn from(value: Error) -> Self {
        InfraError::SerdeJson {
            cause: value.to_string(),
        }
    }
}

impl From<serenity::Error> for InfraError {
    fn from(value: serenity::Error) -> Self {
        InfraError::SerenityError {
            cause: value.to_string(),
        }
    }
}
