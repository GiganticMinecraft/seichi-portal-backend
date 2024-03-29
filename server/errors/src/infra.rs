use thiserror::Error;

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
    FormNotFound { id: i32 },
    #[error("Answer Not Fount: id = {}", .id)]
    AnswerNotFount { id: i32 },
    #[error("Forbidden")]
    Forbidden,
    #[error("Outgoing Error: {}", .cause)]
    Outgoing { cause: String },
    #[error("Enum Parse Error: source = {}", .source)]
    EnumParse {
        #[from]
        source: strum::ParseError,
    },
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
