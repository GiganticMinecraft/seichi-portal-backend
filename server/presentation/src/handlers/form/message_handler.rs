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

#[utoipa::path(
    post,
    path = "/forms/{form_id}/answers/{answer_id}/messages",
    summary = "メッセージの作成",
    params(
        ("form_id" = String, Path, description = "Form ID"),
        ("answer_id" = String, Path, description = "Answer ID"),
    ),
    request_body = PostedMessageSchema,
    responses(
        (status = 200),
    ),
    security(("bearer" = [])),
    tag = "Messages"
)]
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

#[utoipa::path(
    patch,
    path = "/forms/{form_id}/answers/{answer_id}/messages/{message_id}",
    summary = "メッセージの編集",
    params(
        ("form_id" = String, Path, description = "Form ID"),
        ("answer_id" = String, Path, description = "Answer ID"),
        ("message_id" = String, Path, description = "Message ID"),
    ),
    request_body = MessageUpdateSchema,
    responses(
        (status = 204),
    ),
    security(("bearer" = [])),
    tag = "Messages"
)]
pub async fn update_message_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId, MessageId)>, PathRejection>,
    json: Result<Json<MessageUpdateSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_message_use_case = MessageUseCase {
        message_repository: repository.form_message_repository(),
        answer_repository: repository.form_answer_repository(),
        notification_repository: repository.notification_repository(),
        form_repository: repository.form_repository(),
        user_repository: repository.user_repository(),
    };

    let Path((form_id, answer_id, message_id)) = path.map_err_to_error().map_err(handle_error)?;
    let Json(body_schema) = json.map_err_to_error().map_err(handle_error)?;

    form_message_use_case
        .update_message_body(&user, form_id, answer_id, &message_id, body_schema.body)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

#[utoipa::path(
    get,
    path = "/forms/{form_id}/answers/{answer_id}/messages",
    summary = "メッセージの取得",
    params(
        ("form_id" = String, Path, description = "Form ID"),
        ("answer_id" = String, Path, description = "Answer ID"),
    ),
    responses(
        (status = 200, body = Vec<MessageContentSchema>),
    ),
    security(("bearer" = [])),
    tag = "Messages"
)]
pub async fn get_messages_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId)>, PathRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_message_use_case = MessageUseCase {
        message_repository: repository.form_message_repository(),
        answer_repository: repository.form_answer_repository(),
        notification_repository: repository.notification_repository(),
        form_repository: repository.form_repository(),
        user_repository: repository.user_repository(),
    };

    let Path((form_id, answer_id)) = path.map_err_to_error().map_err(handle_error)?;

    let messages = form_message_use_case
        .get_messages(&user, form_id, answer_id)
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

#[utoipa::path(
    delete,
    path = "/forms/{form_id}/answers/{answer_id}/messages/{message_id}",
    summary = "メッセージの削除",
    params(
        ("form_id" = String, Path, description = "Form ID"),
        ("answer_id" = String, Path, description = "Answer ID"),
        ("message_id" = String, Path, description = "Message ID"),
    ),
    responses(
        (status = 204),
    ),
    security(("bearer" = [])),
    tag = "Messages"
)]
pub async fn delete_message_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId, MessageId)>, PathRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_message_use_case = MessageUseCase {
        message_repository: repository.form_message_repository(),
        answer_repository: repository.form_answer_repository(),
        notification_repository: repository.notification_repository(),
        form_repository: repository.form_repository(),
        user_repository: repository.user_repository(),
    };

    let Path((form_id, answer_id, message_id)) = path.map_err_to_error().map_err(handle_error)?;

    form_message_use_case
        .delete_message(&user, form_id, answer_id, &message_id)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
