use axum::response::Response;
use axum::{
    Json,
    http::{StatusCode, header},
    response::IntoResponse,
};
use errors::presentation::PresentationError;
use errors::{
    Error, domain::DomainError, infra::InfraError, usecase::UseCaseError,
    validation::ValidationError,
};
use serde_json::json;

fn problem_response(status: StatusCode, title: &str, detail: &str, error_code: &str) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "application/problem+json")],
        Json(json!({
            "type": "about:blank",
            "title": title,
            "status": status.as_u16(),
            "detail": detail,
            "errorCode": error_code,
        })),
    )
        .into_response()
}

fn handle_domain_error(err: DomainError) -> impl IntoResponse {
    match err {
        DomainError::Forbidden => problem_response(
            StatusCode::FORBIDDEN,
            "Forbidden",
            "You do not have permission to access this resource.",
            "FORBIDDEN",
        ),
        DomainError::NotFound => problem_response(
            StatusCode::NOT_FOUND,
            "Not Found",
            "Resource not found.",
            "NOT_FOUND",
        ),
        DomainError::EmptyMessageBody => problem_response(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "Message body is empty.",
            "EMPTY_MESSAGE_BODY",
        ),
        DomainError::Conversion { source } => {
            tracing::error!("Conversion Error: {}", source);
            problem_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                "Conversion Error",
                "INTERNAL_SERVER_ERROR",
            )
        }
        DomainError::InvalidResponsePeriod => problem_response(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "Invalid response period.",
            "INVALID_RESPONSE_PERIOD",
        ),
        DomainError::InvalidWebhookUrl => problem_response(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "Invalid webhook url. (Seichi-Portal only supports Discord webhook)",
            "INVALID_WEBHOOK_URL",
        ),
    }
}

fn handle_usecase_error(err: UseCaseError) -> impl IntoResponse {
    match err {
        UseCaseError::AnswerNotFound => problem_response(
            StatusCode::NOT_FOUND,
            "Not Found",
            "Answer not found.",
            "ANSWER_NOT_FOUND",
        ),
        UseCaseError::OutOfPeriod => problem_response(
            StatusCode::FORBIDDEN,
            "Forbidden",
            "Posted forms is out of period.",
            "OUT_OF_PERIOD",
        ),
        UseCaseError::MessageNotFound => problem_response(
            StatusCode::NOT_FOUND,
            "Not Found",
            "Message not found.",
            "MESSAGE_NOT_FOUND",
        ),
        UseCaseError::FormNotFound => problem_response(
            StatusCode::NOT_FOUND,
            "Not Found",
            "Form not found.",
            "FORM_NOT_FOUND",
        ),
        UseCaseError::NotificationNotFound => problem_response(
            StatusCode::NOT_FOUND,
            "Not Found",
            "Notification not found.",
            "NOTIFICATION_NOT_FOUND",
        ),
        UseCaseError::LabelNotFound => problem_response(
            StatusCode::NOT_FOUND,
            "Not Found",
            "Label not found.",
            "LABEL_NOT_FOUND",
        ),
        UseCaseError::CommentNotFound => problem_response(
            StatusCode::NOT_FOUND,
            "Not Found",
            "Comment not found.",
            "COMMENT_NOT_FOUND",
        ),
        UseCaseError::DiscordLinkFailed => problem_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Internal Server Error",
            "Failed to link discord.",
            "DISCORD_LINK_FAILED",
        ),
        UseCaseError::UserNotFound => problem_response(
            StatusCode::NOT_FOUND,
            "Not Found",
            "User not found.",
            "USER_NOT_FOUND",
        ),
        UseCaseError::DiscordNotLinked => problem_response(
            StatusCode::FORBIDDEN,
            "Forbidden",
            "Discord is not linked.",
            "DISCORD_NOT_LINKED",
        ),
    }
}

fn handle_infra_error(err: InfraError) -> impl IntoResponse {
    match err {
        InfraError::Database { source } => {
            tracing::error!("Database Error: {}", source);
            problem_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                "Database Error",
                "INTERNAL_SERVER_ERROR",
            )
        }
        InfraError::DatabaseTransaction { cause } => {
            tracing::error!("Transaction Error: {}", cause);
            problem_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                "Transaction Error",
                "INTERNAL_SERVER_ERROR",
            )
        }
        InfraError::UuidParse { source } => {
            tracing::error!("Uuid Parse Error: {}", source);
            problem_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                "Uuid Parse Error",
                "INTERNAL_SERVER_ERROR",
            )
        }
        InfraError::FormNotFound { id } => {
            tracing::error!("Form Not Found: id = {}", id);
            problem_response(
                StatusCode::NOT_FOUND,
                "Not Found",
                "Form not found.",
                "FORM_NOT_FOUND",
            )
        }
        InfraError::AnswerNotFount { id } => {
            tracing::error!("Answer Not Found: id = {}", id);
            problem_response(
                StatusCode::NOT_FOUND,
                "Not Found",
                "Answer not found.",
                "ANSWER_NOT_FOUND",
            )
        }
        InfraError::Outgoing { cause } => {
            tracing::error!("Outgoing Error: {}", cause);
            problem_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                "Outgoing Error",
                "INTERNAL_SERVER_ERROR",
            )
        }
        InfraError::EnumParse { source } => {
            tracing::error!("Enum Parse Error: source = {}", source);
            problem_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                "Enum Parse Error",
                "INTERNAL_SERVER_ERROR",
            )
        }
        InfraError::Redis { source } => {
            tracing::error!("Redis Error: {}", source);
            problem_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                "Database Error",
                "INTERNAL_SERVER_ERROR",
            )
        }
        InfraError::Reqwest { cause } => {
            tracing::error!("Reqwest Error: {}", cause);
            problem_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                "HTTP request Error",
                "INTERNAL_SERVER_ERROR",
            )
        }
        InfraError::MeiliSearch { cause } => {
            tracing::error!("MeiliSearch Error: {}", cause);
            problem_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                "Search database Error",
                "INTERNAL_SERVER_ERROR",
            )
        }
        InfraError::SerdeJson { cause } => {
            tracing::error!("SerdeJson Error: {}", cause);
            problem_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                "JSON parse Error",
                "INTERNAL_SERVER_ERROR",
            )
        }
        InfraError::SerenityError { cause } => {
            tracing::error!("Serenity Error: {}", cause);
            problem_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                "Discord API Error",
                "INTERNAL_SERVER_ERROR",
            )
        }
        InfraError::AMQP { source } => {
            tracing::error!("AMQP Error: {}", source);
            problem_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                "AMQP Error",
                "INTERNAL_SERVER_ERROR",
            )
        }
        InfraError::Send { cause } => {
            tracing::error!("Send Error: {}", cause);
            problem_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                "Send Error",
                "INTERNAL_SERVER_ERROR",
            )
        }
    }
}

fn handle_validation_error(err: ValidationError) -> impl IntoResponse {
    match err {
        ValidationError::EmptyValue => problem_response(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "Empty value error.",
            "EMPTY_VALUE",
        ),
        ValidationError::NegativeValue => problem_response(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "Negative value error.",
            "NEGATIVE_VALUE",
        ),
    }
}

fn handle_presentation_error(err: PresentationError) -> impl IntoResponse {
    match err {
        PresentationError::JsonRejection { cause } => problem_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Unprocessable Content",
            &cause,
            "UNPROCESSABLE_CONTENT",
        ),
        PresentationError::PathRejection { cause } => problem_response(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            &cause,
            "BAD_REQUEST",
        ),
        PresentationError::QueryRejection { cause } => problem_response(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            &cause,
            "BAD_REQUEST",
        ),
        PresentationError::TypedHeaderRejection { cause } => problem_response(
            StatusCode::UNAUTHORIZED,
            "Unauthorized",
            &cause,
            "UNAUTHORIZED",
        ),
    }
}

pub fn handle_error(err: Error) -> Response {
    match err {
        Error::Domain { source } => handle_domain_error(source).into_response(),
        Error::UseCase { source } => handle_usecase_error(source).into_response(),
        Error::Infra { source } => handle_infra_error(source).into_response(),
        Error::Validation { source } => handle_validation_error(source).into_response(),
        Error::Presentation { source } => handle_presentation_error(source).into_response(),
    }
}
