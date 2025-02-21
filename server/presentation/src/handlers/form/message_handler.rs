use axum::{
    Extension, Json,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::IntoResponse,
};
use domain::{
    form::{answer::models::AnswerId, message::models::MessageId},
    repository::Repositories,
    user::models::User,
};
use itertools::Itertools;
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::forms::message::MessageUseCase;

use crate::{
    handlers::error_handler::handle_error,
    schemas::form::{
        form_request_schemas::{MessageUpdateSchema, PostedMessageSchema},
        form_response_schemas::{MessageContentSchema, SenderSchema},
    },
};

pub async fn post_message_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path(answer_id): Path<AnswerId>,
    Json(message): Json<PostedMessageSchema>,
) -> impl IntoResponse {
    let form_message_use_case = MessageUseCase {
        message_repository: repository.form_message_repository(),
        answer_repository: repository.form_answer_repository(),
        notification_repository: repository.notification_repository(),
        form_repository: repository.form_repository(),
        user_repository: repository.user_repository(),
    };

    match form_message_use_case
        .post_message(&user, message.body, answer_id)
        .await
    {
        Ok(_) => (
            StatusCode::CREATED,
            [(
                header::LOCATION,
                HeaderValue::from_str(answer_id.to_string().as_str()).unwrap(),
            )],
        )
            .into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn update_message_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path((answer_id, message_id)): Path<(AnswerId, MessageId)>,
    Json(body_schema): Json<MessageUpdateSchema>,
) -> impl IntoResponse {
    let form_message_use_case = MessageUseCase {
        message_repository: repository.form_message_repository(),
        answer_repository: repository.form_answer_repository(),
        notification_repository: repository.notification_repository(),
        form_repository: repository.form_repository(),
        user_repository: repository.user_repository(),
    };

    match form_message_use_case
        .update_message_body(&user, answer_id, &message_id, body_schema.body)
        .await
    {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn get_messages_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path(answer_id): Path<AnswerId>,
) -> impl IntoResponse {
    let form_message_use_case = MessageUseCase {
        message_repository: repository.form_message_repository(),
        answer_repository: repository.form_answer_repository(),
        notification_repository: repository.notification_repository(),
        form_repository: repository.form_repository(),
        user_repository: repository.user_repository(),
    };

    match form_message_use_case.get_messages(&user, answer_id).await {
        Ok(messages) => {
            let response = messages
                .into_iter()
                .map(|message| MessageContentSchema {
                    id: message.id().into_inner(),
                    body: message.body().to_owned(),
                    sender: SenderSchema {
                        uuid: message.sender().id.to_string(),
                        name: message.sender().name.to_owned(),
                        role: message.sender().role.to_string(),
                    },
                    timestamp: message.timestamp().to_owned(),
                })
                .collect_vec();

            (StatusCode::OK, Json(json!(response))).into_response()
        }
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn delete_message_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path((answer_id, message_id)): Path<(AnswerId, MessageId)>,
) -> impl IntoResponse {
    let form_message_use_case = MessageUseCase {
        message_repository: repository.form_message_repository(),
        answer_repository: repository.form_answer_repository(),
        notification_repository: repository.notification_repository(),
        form_repository: repository.form_repository(),
        user_repository: repository.user_repository(),
    };

    match form_message_use_case
        .delete_message(&user, answer_id, &message_id)
        .await
    {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}
