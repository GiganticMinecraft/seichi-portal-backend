use axum::{
    extract::{Path, State},
    response::IntoResponse,
    Extension, Json,
};
use domain::{
    form::models::AnswerId, message::models::Message, repository::Repositories, user::models::User,
};
use errors::{domain::DomainError, Error};
use reqwest::StatusCode;
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use types::Resolver;
use usecase::message::MessageUseCase;

use crate::message_schemas::PostedMessageSchema;

pub async fn post_message_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Json(message): Json<PostedMessageSchema>,
) -> impl IntoResponse {
    let message_use_case = MessageUseCase {
        message_repository: repository.message_repository(),
        form_repository: repository.form_repository(),
    };

    // TODO: ここで related_answer_id を取得しているのはおかしいかもしれない
    let related_answer = message
        .related_answer_id
        .resolve(repository.form_repository())
        .await;

    let answer = match related_answer {
        Ok(Some(related_answer)) => related_answer,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "errorCode": "NOT_FOUND",
                    "reason": "Answer not found."
                })),
            )
                .into_response();
        }
        Err(err) => {
            return handle_error(err).into_response();
        }
    };

    let new_message = match Message::try_new(answer, user, message.body) {
        Ok(new_message) => new_message,
        Err(DomainError::Forbidden) => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({
                    "errorCode": "FORBIDDEN",
                    "reason": "You cannot access to this message."
                })),
            )
                .into_response();
        }
        Err(err) => {
            return handle_error(Into::into(err)).into_response();
        }
    };

    match message_use_case.post_message(&new_message).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn get_messages_handler(
    State(repository): State<RealInfrastructureRepository>,
    Path(answer_id): Path<AnswerId>,
) -> impl IntoResponse {
    let message_use_case = MessageUseCase {
        message_repository: repository.message_repository(),
        form_repository: repository.form_repository(),
    };

    match message_use_case.get_message(answer_id).await {
        Ok(messages) => (StatusCode::OK, Json(json!(messages))).into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub fn handle_error(err: Error) -> impl IntoResponse {
    {
        tracing::error!("{}", err);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "errorCode": "INTERNAL_SERVER_ERROR",
                "reason": "unknown error"
            })),
        )
            .into_response()
    }
}
