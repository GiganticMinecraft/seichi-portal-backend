use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum DomainError {
    #[error("Conversion Error: {}", .source)]
    Conversion {
        #[from]
        source: strum::ParseError,
    },
    #[error("Access to forbidden resource.")]
    Forbidden,
    #[error("Empty message body.")]
    EmptyMessageBody,
    #[error("Empty value error.")]
    EmptyValue,
    #[error("Invalid response period.")]
    InvalidResponsePeriod,
    #[error("Invalid webhook url.")]
    InvalidWebhookUrl,
}
