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

use crate::schemas::error_response::{ErrorResponse, ErrorRestriction};

pub struct ApiError {
    status: StatusCode,
    title: &'static str,
    detail: String,
    error_code: &'static str,
    restriction: Option<ErrorRestriction>,
}

impl ApiError {
    pub(crate) fn unauthorized(detail: &str) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            title: "Unauthorized",
            detail: detail.to_owned(),
            error_code: "UNAUTHORIZED",
            restriction: None,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status,
            [(header::CONTENT_TYPE, "application/problem+json")],
            Json(ErrorResponse {
                problem_type: "about:blank".to_string(),
                title: self.title.to_string(),
                status: self.status.as_u16(),
                detail: self.detail,
                error_code: self.error_code.to_string(),
                restriction: self.restriction,
            }),
        )
            .into_response()
    }
}

fn problem_response(
    status: StatusCode,
    title: &'static str,
    detail: impl Into<String>,
    error_code: &'static str,
) -> ApiError {
    problem_response_with_restriction(status, title, detail, error_code, None)
}

fn problem_response_with_restriction(
    status: StatusCode,
    title: &'static str,
    detail: impl Into<String>,
    error_code: &'static str,
    restriction: Option<ErrorRestriction>,
) -> ApiError {
    ApiError {
        status,
        title,
        detail: detail.into(),
        error_code,
        restriction,
    }
}

fn handle_domain_error(err: DomainError) -> ApiError {
    match err {
        DomainError::Forbidden => problem_response(
            StatusCode::FORBIDDEN,
            "Forbidden",
            "You do not have permission to access this resource.",
            "FORBIDDEN",
        ),
        DomainError::SubmissionRestricted { reason, expires_at } => {
            problem_response_with_restriction(
                StatusCode::FORBIDDEN,
                "Forbidden",
                "Form submission is restricted.",
                "SUBMISSION_RESTRICTED",
                Some(ErrorRestriction { reason, expires_at }),
            )
        }
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
        DomainError::MessagePostingNotSupportedForTemporaryAnswer => problem_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Unprocessable Entity",
            "Messages cannot be posted to temporary answers.",
            "MESSAGE_POSTING_NOT_SUPPORTED_FOR_TEMPORARY_ANSWER",
        ),
        DomainError::MessagePostingNotSupportedForImportedAnswer => problem_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Unprocessable Entity",
            "Messages cannot be posted to answers imported from Redmine.",
            "MESSAGE_POSTING_NOT_SUPPORTED_FOR_IMPORTED_ANSWER",
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
        DomainError::InvalidAnswerAcceptancePeriod => problem_response(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "Invalid answer acceptance period.",
            "INVALID_ANSWER_ACCEPTANCE_PERIOD",
        ),
        DomainError::InvalidDiscordWebhookUrl => problem_response(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "Invalid Discord webhook url. (Seichi-Portal only supports Discord webhook)",
            "INVALID_DISCORD_WEBHOOK_URL",
        ),
        DomainError::InvalidEntity { message } => problem_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Unprocessable Entity",
            message,
            "INVALID_ENTITY",
        ),
    }
}

fn handle_usecase_error(err: UseCaseError) -> ApiError {
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
        UseCaseError::UserGroupNotFound => problem_response(
            StatusCode::NOT_FOUND,
            "Not Found",
            "User group not found.",
            "USER_GROUP_NOT_FOUND",
        ),
        UseCaseError::DiscordNotLinked => problem_response(
            StatusCode::FORBIDDEN,
            "Forbidden",
            "Discord is not linked.",
            "DISCORD_NOT_LINKED",
        ),
    }
}

fn handle_infra_error(err: InfraError) -> ApiError {
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
        InfraError::Unexpected { cause } => {
            tracing::error!("Unexpected Error: {}", cause);
            problem_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                "Unexpected Error",
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
                StatusCode::SERVICE_UNAVAILABLE,
                "Service Unavailable",
                "Search service is temporarily unavailable.",
                "SEARCH_SERVICE_UNAVAILABLE",
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

fn handle_validation_error(err: ValidationError) -> ApiError {
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
        ValidationError::DuplicateElement => problem_response(
            StatusCode::BAD_REQUEST,
            "Bad Request",
            "Duplicate element error.",
            "DUPLICATE_ELEMENT",
        ),
    }
}

fn handle_presentation_error(err: PresentationError) -> ApiError {
    match err {
        PresentationError::JsonRejection { cause } => problem_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Unprocessable Content",
            cause,
            "UNPROCESSABLE_CONTENT",
        ),
        PresentationError::PathRejection { cause } => {
            problem_response(StatusCode::BAD_REQUEST, "Bad Request", cause, "BAD_REQUEST")
        }
        PresentationError::QueryRejection { cause } => {
            problem_response(StatusCode::BAD_REQUEST, "Bad Request", cause, "BAD_REQUEST")
        }
        PresentationError::TypedHeaderRejection { cause } => problem_response(
            StatusCode::UNAUTHORIZED,
            "Unauthorized",
            cause,
            "UNAUTHORIZED",
        ),
    }
}

pub fn handle_error(err: Error) -> ApiError {
    match err {
        Error::Domain { source } => handle_domain_error(source),
        Error::UseCase { source } => handle_usecase_error(source),
        Error::Infra { source } => handle_infra_error(source),
        Error::Validation { source } => handle_validation_error(source),
        Error::Presentation { source } => handle_presentation_error(source),
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::to_bytes, http::header::CONTENT_TYPE};

    use super::*;

    #[tokio::test]
    async fn temporary_answer_message_posting_error_is_an_unprocessable_entity_problem() {
        let response =
            handle_error(DomainError::MessagePostingNotSupportedForTemporaryAnswer.into())
                .into_response();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            response.headers().get(CONTENT_TYPE).unwrap(),
            "application/problem+json"
        );

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let problem: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(problem["status"], 422);
        assert_eq!(problem["title"], "Unprocessable Entity");
        assert_eq!(
            problem["errorCode"],
            "MESSAGE_POSTING_NOT_SUPPORTED_FOR_TEMPORARY_ANSWER"
        );
        assert_eq!(
            problem["detail"],
            "Messages cannot be posted to temporary answers."
        );
        assert!(problem.get("restriction").is_none());
    }

    #[tokio::test]
    async fn submission_restriction_error_includes_restriction_details() {
        let response = handle_error(
            DomainError::SubmissionRestricted {
                reason: "spam".to_string(),
                expires_at: None,
            }
            .into(),
        )
        .into_response();

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let problem: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(problem["status"], 403);
        assert_eq!(problem["errorCode"], "SUBMISSION_RESTRICTED");
        assert_eq!(problem["restriction"]["reason"], "spam");
        assert!(problem["restriction"].get("expires_at").is_some());
    }

    #[tokio::test]
    async fn meilisearch_error_is_a_search_service_unavailable_problem() {
        let response = handle_error(
            InfraError::MeiliSearch {
                cause: "connection refused".to_string(),
            }
            .into(),
        )
        .into_response();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            response.headers().get(CONTENT_TYPE).unwrap(),
            "application/problem+json"
        );

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let problem: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(problem["status"], 503);
        assert_eq!(problem["title"], "Service Unavailable");
        assert_eq!(problem["errorCode"], "SEARCH_SERVICE_UNAVAILABLE");
        assert_eq!(
            problem["detail"],
            "Search service is temporarily unavailable."
        );
    }
}
