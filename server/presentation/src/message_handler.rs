use axum::{extract::State, response::IntoResponse, Extension, Json};
use domain::{message::models::Message, repository::Repositories, user::models::User};
use errors::Error;
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
        repository: repository.message_repository(),
    };

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

    let new_message = match Message::new(answer, user, message.body) {
        Ok(new_message) => new_message,
        Err(err) => {
            return handle_error(Into::into(err)).into_response();
        }
    };

    match message_use_case.post_message(&new_message).await {
        Ok(_) => StatusCode::OK.into_response(),
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
