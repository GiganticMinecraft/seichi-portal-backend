use serde_json::Error;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum InfraError {
    #[error("Database Error: {}", .source)]
    Database {
        #[from]
        source: sqlx::Error,
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
    #[error("Serenity Error: {}", .cause)]
    SerenityError { cause: String },
    #[error("AMQP Error: {}", .source)]
    AMQP {
        #[from]
        source: lapin::Error,
    },
    #[error("Send Error: {}", .cause)]
    Send { cause: String },
}

impl PartialEq for InfraError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Database { source: left }, Self::Database { source: right }) => {
                left.to_string() == right.to_string()
            }
            (
                Self::DatabaseTransaction { cause: left },
                Self::DatabaseTransaction { cause: right },
            ) => left == right,
            (Self::UuidParse { source: left }, Self::UuidParse { source: right }) => left == right,
            (Self::FormNotFound { id: left }, Self::FormNotFound { id: right }) => left == right,
            (Self::AnswerNotFount { id: left }, Self::AnswerNotFount { id: right }) => {
                left == right
            }
            (Self::Outgoing { cause: left }, Self::Outgoing { cause: right }) => left == right,
            (Self::EnumParse { source: left }, Self::EnumParse { source: right }) => left == right,
            (Self::Redis { source: left }, Self::Redis { source: right }) => {
                left.to_string() == right.to_string()
            }
            (Self::Reqwest { cause: left }, Self::Reqwest { cause: right }) => left == right,
            (Self::MeiliSearch { cause: left }, Self::MeiliSearch { cause: right }) => {
                left == right
            }
            (Self::SerdeJson { cause: left }, Self::SerdeJson { cause: right }) => left == right,
            (Self::SerenityError { cause: left }, Self::SerenityError { cause: right }) => {
                left == right
            }
            (Self::AMQP { source: left }, Self::AMQP { source: right }) => {
                left.to_string() == right.to_string()
            }
            (Self::Send { cause: left }, Self::Send { cause: right }) => left == right,
            _ => false,
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

impl<T> From<tokio::sync::mpsc::error::SendError<T>> for InfraError {
    fn from(value: tokio::sync::mpsc::error::SendError<T>) -> Self {
        InfraError::Send {
            cause: value.to_string(),
        }
    }
}

impl From<reqwest::Error> for InfraError {
    fn from(value: reqwest::Error) -> Self {
        InfraError::Reqwest {
            cause: value.to_string(),
        }
    }
}
