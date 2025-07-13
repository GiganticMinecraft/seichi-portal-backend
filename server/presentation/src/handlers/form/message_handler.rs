use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
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
use std::sync::Arc;
use usecase::forms::message::MessageUseCase;

use crate::{
    handlers::error_handler::handle_error,
    schemas::form::{
        form_request_schemas::{MessageUpdateSchema, PostedMessageSchema},
        form_response_schemas::{MessageContentSchema, SenderSchema},
    },
};
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::response::Response;
use domain::form::models::FormId;
use domain::notification::notification_api::NotificationAPI;
use errors::ErrorExtra;

pub struct RealInfrastructureRepositoryWithNotificationAPI<API: NotificationAPI + Send + Sync> {
    pub repository: RealInfrastructureRepository,
    pub notification_api: API,
}

impl<API: NotificationAPI + Send + Sync> RealInfrastructureRepositoryWithNotificationAPI<API> {
    pub const fn new(repository: RealInfrastructureRepository, notification_api: API) -> Self {
        Self {
            repository,
            notification_api,
        }
    }
}

pub async fn post_message_handler<API: NotificationAPI + Send + Sync>(
    Extension(user): Extension<User>,
    State(state): State<Arc<RealInfrastructureRepositoryWithNotificationAPI<API>>>,
    path: Result<Path<(FormId, AnswerId)>, PathRejection>,
    json: Result<Json<PostedMessageSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_message_use_case = MessageUseCase {
        message_repository: state.repository.form_message_repository(),
        answer_repository: state.repository.form_answer_repository(),
        notification_repository: state.repository.notification_repository(),
        form_repository: state.repository.form_repository(),
        user_repository: state.repository.user_repository(),
    };

    let Path((form_id, answer_id)) = path.map_err_to_error().map_err(handle_error)?;
    let Json(message) = json.map_err_to_error().map_err(handle_error)?;

    form_message_use_case
        .post_message(
            &user,
            form_id,
            message.body,
            answer_id,
            &state.notification_api,
        )
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::OK.into_response())
}

pub async fn update_message_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path((form_id, answer_id, message_id)): Path<(FormId, AnswerId, MessageId)>,
    json: Result<Json<MessageUpdateSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_message_use_case = MessageUseCase {
        message_repository: repository.form_message_repository(),
        answer_repository: repository.form_answer_repository(),
        notification_repository: repository.notification_repository(),
        form_repository: repository.form_repository(),
        user_repository: repository.user_repository(),
    };

    let Json(body_schema) = json.map_err_to_error().map_err(handle_error)?;

    form_message_use_case
        .update_message_body(&user, form_id, answer_id, &message_id, body_schema.body)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

pub async fn get_messages_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path(answer_id): Path<AnswerId>,
) -> Result<impl IntoResponse, Response> {
    let form_message_use_case = MessageUseCase {
        message_repository: repository.form_message_repository(),
        answer_repository: repository.form_answer_repository(),
        notification_repository: repository.notification_repository(),
        form_repository: repository.form_repository(),
        user_repository: repository.user_repository(),
    };

    let messages = form_message_use_case
        .get_messages(&user, answer_id)
        .await
        .map_err(handle_error)?;
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

    Ok((StatusCode::OK, Json(json!(response))).into_response())
}

pub async fn delete_message_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path((form_id, answer_id, message_id)): Path<(FormId, AnswerId, MessageId)>,
) -> Result<impl IntoResponse, Response> {
    let form_message_use_case = MessageUseCase {
        message_repository: repository.form_message_repository(),
        answer_repository: repository.form_answer_repository(),
        notification_repository: repository.notification_repository(),
        form_repository: repository.form_repository(),
        user_repository: repository.user_repository(),
    };

    form_message_use_case
        .delete_message(&user, form_id, answer_id, &message_id)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
