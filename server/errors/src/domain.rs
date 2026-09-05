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
    #[error("Form submission is restricted.")]
    SubmissionRestricted {
        reason: String,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    },
    #[error("Empty message body.")]
    EmptyMessageBody,
    #[error("Messages cannot be posted to temporary answers.")]
    MessagePostingNotSupportedForTemporaryAnswer,
    #[error("Messages cannot be posted to answers imported from Redmine.")]
    MessagePostingNotSupportedForImportedAnswer,
    #[error("Invalid answer acceptance period.")]
    InvalidAnswerAcceptancePeriod,
    #[error("Invalid Discord webhook url.")]
    InvalidDiscordWebhookUrl,
    #[error("Invalid session expiration.")]
    InvalidSessionExpiration,
    #[error("Invalid entity: {message}")]
    InvalidEntity { message: String },
}
