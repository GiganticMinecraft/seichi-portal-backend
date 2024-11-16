use axum::{http::StatusCode, response::IntoResponse, Json};
use errors::{domain::DomainError, infra::InfraError, usecase::UseCaseError, Error};
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
                "reason": "Posted form is out of period."
            })),
        )
            .into_response(),
        UseCaseError::DoNotHavePermissionToPostFormComment => (
            StatusCode::FORBIDDEN,
            Json(json!({
                "errorCode": "DO_NOT_HAVE_PERMISSION_TO_POST_FORM_COMMENT",
                "reason": "Do not have permission to post form comment."
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
    }
}

pub fn handle_error(err: Error) -> impl IntoResponse {
    match err {
        Error::Domain { source } => handle_domain_error(source).into_response(),
        Error::UseCase { source } => handle_usecase_error(source).into_response(),
        Error::Infra { source } => handle_infra_error(source).into_response(),
    }
}
