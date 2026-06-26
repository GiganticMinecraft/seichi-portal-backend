use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum DomainError {
    #[error("Conversion Error: {}", .source)]
    Conversion {
        #[from]
        source: strum::ParseError,
    },
    #[error("Not found.")]
    NotFound,
    #[error("Access to forbidden resource.")]
    Forbidden,
    #[error("Answer submission is restricted.")]
    AnswerSubmissionRestricted {
        reason: String,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    },
    #[error("Empty message body.")]
    EmptyMessageBody,
    #[error("Invalid answer acceptance period.")]
    InvalidAnswerAcceptancePeriod,
    #[error("Invalid Discord webhook url.")]
    InvalidDiscordWebhookUrl,
    #[error("Invalid entity: {message}")]
    InvalidEntity { message: String },
}
