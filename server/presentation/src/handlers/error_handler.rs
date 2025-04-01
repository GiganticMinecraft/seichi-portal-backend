use axum::{Json, http::StatusCode, response::IntoResponse};
use errors::{
    Error, domain::DomainError, infra::InfraError, usecase::UseCaseError,
    validation::ValidationError,
};
use serde_json::json;

fn handle_domain_error(err: DomainError) -> impl IntoResponse {
    match err {
        DomainError::Forbidden => (
            StatusCode::FORBIDDEN,
            Json(json!({
                "errorCode": "FORBIDDEN",
                "reason": "You do not have permission to access this resource."
            })),
        )
            .into_response(),
        DomainError::NotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "errorCode": "NOT_FOUND",
                "reason": "Resource not found."
            })),
        )
            .into_response(),
        DomainError::EmptyMessageBody => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "errorCode": "EMPTY_MESSAGE_BODY",
                "reason": "Message body is empty."
            })),
        )
            .into_response(),
        DomainError::Conversion { source } => {
            tracing::error!("Conversion Error: {}", source);

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "errorCode": "INTERNAL_SERVER_ERROR",
                    "reason": "Conversion Error",
                })),
            )
                .into_response()
        }
        DomainError::InvalidResponsePeriod => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "errorCode": "INVALID_RESPONSE_PERIOD",
                "reason": "Invalid response period."
            })),
        )
            .into_response(),
        DomainError::InvalidWebhookUrl => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "errorCode": "INVALID_WEBHOOK_URL",
                "reason": "Invalid webhook url. (Seichi-Portal only supports Discord webhook)"
            })),
        )
            .into_response(),
    }
}

fn handle_usecase_error(err: UseCaseError) -> impl IntoResponse {
    match err {
        UseCaseError::AnswerNotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "errorCode": "ANSWER_NOT_FOUND",
                "reason": "Answer not found"
            })),
        )
            .into_response(),
        UseCaseError::OutOfPeriod => (
            StatusCode::FORBIDDEN,
            Json(json!({
                "errorCode": "OUT_OF_PERIOD",
                "reason": "Posted forms is out of period."
            })),
        )
            .into_response(),
        UseCaseError::MessageNotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "errorCode": "MESSAGE_NOT_FOUND",
                "reason": "Message not found"
            })),
        )
            .into_response(),
        UseCaseError::FormNotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "errorCode": "FORM_NOT_FOUND",
                "reason": "FORM NOT FOUND"
            })),
        )
            .into_response(),
        UseCaseError::NotificationNotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "errorCode": "NOTIFICATION_NOT_FOUND",
                "reason": "Notification not found"
            })),
        )
            .into_response(),
        UseCaseError::LabelNotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "errorCode": "LABEL_NOT_FOUND",
                "reason": "Label not found"
            })),
        )
            .into_response(),
        UseCaseError::CommentNotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "errorCode": "COMMENT_NOT_FOUND",
                "reason": "Comment not found"
            })),
        )
            .into_response(),
        UseCaseError::DiscordLinkFailed => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "errorCode": "DISCORD_LINK_FAILED",
                "reason": "Failed to link discord"
            })),
        )
            .into_response(),
        UseCaseError::UserNotFound => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "errorCode": "USER_NOT_FOUND",
                "reason": "User not found"
            })),
        )
            .into_response(),
    }
}

fn handle_infra_error(err: InfraError) -> impl IntoResponse {
    match err {
        InfraError::Database { source } => {
            tracing::error!("Database Error: {}", source);

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "errorCode": "INTERNAL_SERVER_ERROR",
                    "reason": "Database Error",
                })),
            )
                .into_response()
        }
        InfraError::DatabaseTransaction { cause } => {
            tracing::error!("Transaction Error: {}", cause);

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "errorCode": "INTERNAL_SERVER_ERROR",
                    "reason": "Transaction Error",
                })),
            )
                .into_response()
        }
        InfraError::UuidParse { source } => {
            tracing::error!("Uuid Parse Error: {}", source);

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "errorCode": "INTERNAL_SERVER_ERROR",
                    "reason": "Uuid Parse Error",
                })),
            )
                .into_response()
        }
        InfraError::FormNotFound { id } => {
            tracing::error!("Form Not Found: id = {}", id);

            (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "errorCode": "FORM_NOT_FOUND",
                    "reason": "Form not found"
                })),
            )
                .into_response()
        }
        InfraError::AnswerNotFount { id } => {
            tracing::error!("Answer Not Found: id = {}", id);

            (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "errorCode": "ANSWER_NOT_FOUND",
                    "reason": "Answer not found"
                })),
            )
                .into_response()
        }
        InfraError::Outgoing { cause } => {
            tracing::error!("Outgoing Error: {}", cause);

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "errorCode": "INTERNAL_SERVER_ERROR",
                    "reason": "Outgoing Error",
                })),
            )
                .into_response()
        }
        InfraError::EnumParse { source } => {
            tracing::error!("Enum Parse Error: source = {}", source);

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "errorCode": "INTERNAL_SERVER_ERROR",
                    "reason": "Enum Parse Error",
                })),
            )
                .into_response()
        }
        InfraError::Redis { source } => {
            tracing::error!("Redis Error: {}", source);

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "errorCode": "INTERNAL_SERVER_ERROR",
                    "reason": "Database Error",
                })),
            )
                .into_response()
        }
        InfraError::Reqwest { cause } => {
            tracing::error!("Reqwest Error: {}", cause);

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "errorCode": "INTERNAL_SERVER_ERROR",
                    "reason": "HTTP request Error",
                })),
            )
                .into_response()
        }
        InfraError::MeiliSearch { cause } => {
            tracing::error!("MeiliSearch Error: {}", cause);

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "errorCode": "INTERNAL_SERVER_ERROR",
                    "reason": "Search database Error",
                })),
            )
                .into_response()
        }
        InfraError::SerdeJson { cause } => {
            tracing::error!("SerdeJson Error: {}", cause);

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "errorCode": "INTERNAL_SERVER_ERROR",
                    "reason": "JSON parse Error",
                })),
            )
                .into_response()
        }
        InfraError::SerenityError { cause } => {
            tracing::error!("Serenity Error: {}", cause);

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "errorCode": "INTERNAL_SERVER_ERROR",
                    "reason": "Discord API Error",
                })),
            )
                .into_response()
        }
        InfraError::AMQP { source } => {
            tracing::error!("AMQP Error: {}", source);

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "errorCode": "INTERNAL_SERVER_ERROR",
                    "reason": "AMQP Error",
                })),
            )
                .into_response()
        }
        InfraError::Send { cause } => {
            tracing::error!("Send Error: {}", cause);

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "errorCode": "INTERNAL_SERVER_ERROR",
                    "reason": "Send Error",
                })),
            )
                .into_response()
        }
    }
}

pub fn handle_validation_error(err: ValidationError) -> impl IntoResponse {
    match err {
        ValidationError::EmptyValue => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "errorCode": "EMPTY_VALUE",
                "reason": "Empty value error."
            })),
        )
            .into_response(),
        ValidationError::NegativeValue => (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "errorCode": "NEGATIVE_VALUE",
                "reason": "Negative value error."
            })),
        )
            .into_response(),
    }
}

pub fn handle_error(err: Error) -> impl IntoResponse {
    match err {
        Error::Domain { source } => handle_domain_error(source).into_response(),
        Error::UseCase { source } => handle_usecase_error(source).into_response(),
        Error::Infra { source } => handle_infra_error(source).into_response(),
        Error::Validation { source } => handle_validation_error(source).into_response(),
    }
}
