use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum InfraError {
    #[error("Database Error: {}", .source)]
    Database {
        #[from]
        source: sea_orm::error::DbErr,
    },
    #[error("Uuid Parse Error: {}", .cause)]
    UuidParse { cause: String },
    #[error("Form Not Found: id = {}", .id)]
    FormNotFound { id: i32 },
    #[error("Outgoing Error: {}", .cause)]
    Outgoing { cause: String },
}
