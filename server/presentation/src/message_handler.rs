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
use usecase::{form::FormUseCase, message::MessageUseCase};

use crate::message_schemas::{
    GetMessageResponseSchema, MessageContentSchema, PostedMessageSchema, SenderSchema,
};

pub async fn post_message_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Json(message): Json<PostedMessageSchema>,
) -> impl IntoResponse {
    let message_use_case = MessageUseCase {
        message_repository: repository.message_repository(),
        form_repository: repository.form_repository(),
    };

    let form_use_case = FormUseCase {
        repository: repository.form_repository(),
    };

    let answer = match form_use_case.get_answers(message.related_answer_id).await {
        Ok(related_answer) => related_answer,
        Err(err) => {
            return handle_error(err).into_response();
        }
    };

    let new_message_guard = match Message::try_new(answer, user.to_owned(), message.body) {
        Ok(guard) => guard.into_read(),
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

    match new_message_guard.try_read(&user) {
        Ok(message) => match message_use_case.post_message(message).await {
            Ok(_) => StatusCode::OK.into_response(),
            Err(err) => handle_error(err).into_response(),
        },
        Err(err) => handle_error(Into::into(err)).into_response(),
    }
}

pub async fn get_messages_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path(answer_id): Path<AnswerId>,
) -> impl IntoResponse {
    let message_use_case = MessageUseCase {
        message_repository: repository.message_repository(),
        form_repository: repository.form_repository(),
    };

    match message_use_case.get_message(answer_id).await {
        Ok(messages) => {
            let messages_read_result = messages
                .into_iter()
                .map(|message_guard| {
                    message_guard
                        .try_read(&user)
                        .map(|message| MessageContentSchema {
                            body: message.body().to_owned(),
                            sender: SenderSchema {
                                uuid: message.posted_user().id.to_string(),
                                name: message.posted_user().name.to_owned(),
                                role: message.posted_user().role.to_string(),
                            },
                            timestamp: message.timestamp().to_owned(),
                        })
                })
                .collect::<Result<Vec<_>, _>>();

            let response_schema = match messages_read_result {
                Ok(message_content_schemas) => GetMessageResponseSchema {
                    messages: message_content_schemas,
                },
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

            (StatusCode::OK, Json(json!(response_schema))).into_response()
        }
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
