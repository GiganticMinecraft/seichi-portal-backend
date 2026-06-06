use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use domain::{
    form::{answer::models::AnswerId, message::models::MessageId},
    repository::Repositories,
    user::models::ActiveUser,
};
use itertools::Itertools;
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use std::sync::Arc;
use usecase::forms::message::MessageUseCase;

use crate::schemas::error_responses::*;
use crate::{
    handlers::error_handler::handle_error,
    schemas::form::{
        form_request_schemas::{MessageUpdateSchema, PostedMessageSchema},
        form_response_schemas::{MessageContentSchema, SenderSchema},
    },
};

#[derive(utoipa::IntoResponses)]
pub enum GetMessagesResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(Vec<MessageContentSchema>),
}

impl IntoResponse for GetMessagesResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(json!(body))).into_response(),
        }
    }
}
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::response::Response;
use domain::form::models::FormId;
use domain::notification::notificator::Notificator;
use errors::ErrorExtra;

pub struct RealInfrastructureRepositoryWithNotificator<N: Notificator> {
    pub repository: RealInfrastructureRepository,
    pub notificator: N,
}

impl<N: Notificator> RealInfrastructureRepositoryWithNotificator<N> {
    pub fn new(repository: RealInfrastructureRepository, notificator: N) -> Self {
        Self {
            repository,
            notificator,
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
        (status = 200, description = "The request has succeeded."),
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Messages"
)]
pub async fn post_message_handler<N: Notificator>(
    Extension(user): Extension<ActiveUser>,
    State(state): State<Arc<RealInfrastructureRepositoryWithNotificator<N>>>,
    path: Result<Path<(FormId, AnswerId)>, PathRejection>,
    json: Result<Json<PostedMessageSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_message_use_case = MessageUseCase {
        notification_repository: state.repository.notification_repository(),
        active_form_repository: state.repository.active_form_repository(),
        user_repository: state.repository.user_repository(),
        answer_entry_repository: state.repository.answer_entry_repository(),
        message_thread_repository: state.repository.message_thread_repository(),
    };

    let Path((form_id, answer_id)) = path.map_err_to_error().map_err(handle_error)?;
    let Json(message) = json.map_err_to_error().map_err(handle_error)?;

    form_message_use_case
        .post_message(&user, form_id, message.body, answer_id, &state.notificator)
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
        (status = 204, description = "There is no content to send for this request, but the headers may be useful."),
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Messages"
)]
pub async fn update_message_handler(
    Extension(user): Extension<ActiveUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId, MessageId)>, PathRejection>,
    json: Result<Json<MessageUpdateSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_message_use_case = MessageUseCase {
        notification_repository: repository.notification_repository(),
        active_form_repository: repository.active_form_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        message_thread_repository: repository.message_thread_repository(),
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
        GetMessagesResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Messages"
)]
pub async fn get_messages_handler(
    Extension(user): Extension<ActiveUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId)>, PathRejection>,
) -> Result<GetMessagesResponse, Response> {
    let form_message_use_case = MessageUseCase {
        notification_repository: repository.notification_repository(),
        active_form_repository: repository.active_form_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        message_thread_repository: repository.message_thread_repository(),
    };

    let Path((form_id, answer_id)) = path.map_err_to_error().map_err(handle_error)?;

    let messages = form_message_use_case
        .get_messages(&user, form_id, answer_id)
        .await
        .map_err(handle_error)?;

    Ok(GetMessagesResponse::Ok(
        messages
            .into_iter()
            .map(|message_with_sender| MessageContentSchema {
                id: message_with_sender.message.id().into_inner(),
                body: message_with_sender.message.body().to_owned(),
                sender: SenderSchema {
                    uuid: message_with_sender.sender.id().to_string(),
                    name: message_with_sender.sender.name().to_owned(),
                    role: message_with_sender.sender.role().to_string(),
                },
                timestamp: message_with_sender.message.timestamp().to_owned(),
            })
            .collect_vec(),
    ))
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
        (status = 204, description = "There is no content to send for this request, but the headers may be useful."),
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Messages"
)]
pub async fn delete_message_handler(
    Extension(user): Extension<ActiveUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId, MessageId)>, PathRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_message_use_case = MessageUseCase {
        notification_repository: repository.notification_repository(),
        active_form_repository: repository.active_form_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        message_thread_repository: repository.message_thread_repository(),
    };

    let Path((form_id, answer_id, message_id)) = path.map_err_to_error().map_err(handle_error)?;

    form_message_use_case
        .delete_message(&user, form_id, answer_id, &message_id)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::NO_CONTENT.into_response())
}
