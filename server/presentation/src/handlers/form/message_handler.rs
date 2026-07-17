use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use domain::{
    account::models::AccountUser,
    form::{
        answer::AnswerId,
        message::{MessageBody, MessageHistoryPagePosition, MessageId},
    },
    pagination::{PageLimit, PageRequest},
    repository::Repositories,
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
        form_request_schemas::{HistoryListQuery, MessageUpdateSchema, PostedMessageSchema},
        form_response_schemas::{MessageContentSchema, MessageHistoryPageResponse, SenderSchema},
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

#[derive(serde::Deserialize, serde::Serialize)]
struct MessageHistoryCursor {
    after_history_id: uuid::Uuid,
}

fn history_page_request(
    query: HistoryListQuery,
) -> Result<PageRequest<MessageHistoryPagePosition>, Error> {
    let limit = match query.limit {
        Some(limit) => PageLimit::try_new(limit).map_err(|error| {
            Error::from(PresentationError::QueryRejection {
                cause: format!("Invalid limit: {}.", error.value()),
            })
        })?,
        None => PageLimit::default_limit(),
    };
    let after = query
        .cursor
        .as_deref()
        .map(|cursor| {
            let decoded = URL_SAFE_NO_PAD.decode(cursor).map_err(|_| {
                Error::from(PresentationError::QueryRejection {
                    cause: "Invalid cursor.".to_string(),
                })
            })?;
            let cursor: MessageHistoryCursor = serde_json::from_slice(&decoded).map_err(|_| {
                Error::from(PresentationError::QueryRejection {
                    cause: "Invalid cursor.".to_string(),
                })
            })?;
            Ok::<_, Error>(MessageHistoryPagePosition::new(
                cursor.after_history_id.into(),
            ))
        })
        .transpose()?;
    Ok(PageRequest::new(after, limit))
}

fn encode_history_cursor(position: MessageHistoryPagePosition) -> Result<String, Error> {
    let bytes = serde_json::to_vec(&MessageHistoryCursor {
        after_history_id: position.id().into_inner(),
    })
    .map_err(|_| {
        Error::from(PresentationError::QueryRejection {
            cause: "Invalid cursor.".to_string(),
        })
    })?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::response::Response;
use domain::form::models::FormId;
use domain::notification::notificator::Notificator;
use errors::{Error, ErrorExtra, presentation::PresentationError};

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
    get,
    path = "/forms/{form_id}/answers/{answer_id}/messages/history",
    summary = "メッセージの変更履歴を取得",
    params(("form_id" = String, Path), ("answer_id" = String, Path), HistoryListQuery),
    responses((status = 200, body = MessageHistoryPageResponse), BadRequest, Unauthorized, Forbidden, NotFound, InternalServerError),
    security(("bearer" = [])),
    tag = "Messages"
)]
pub async fn get_message_history(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId)>, PathRejection>,
    query: Query<HistoryListQuery>,
) -> Result<Json<MessageHistoryPageResponse>, Response> {
    let use_case = MessageUseCase {
        notification_repository: repository.notification_repository(),
        active_form_repository: repository.active_form_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        message_thread_repository: repository.message_thread_repository(),
    };
    let Path((form_id, answer_id)) = path.map_err_to_error().map_err(handle_error)?;
    let request = history_page_request(query.0).map_err(handle_error)?;
    let page = use_case
        .get_history(&user, form_id, answer_id, request)
        .await
        .map_err(handle_error)?;
    let (items, next) = page.into_parts();
    Ok(Json(MessageHistoryPageResponse {
        items: items
            .into_iter()
            .map(|entry| entry.into_inner().into())
            .collect(),
        next_cursor: next
            .map(encode_history_cursor)
            .transpose()
            .map_err(handle_error)?,
    }))
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
    Extension(user): Extension<AccountUser>,
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
        .post_message(
            &user,
            form_id,
            MessageBody::new(message.body),
            answer_id,
            &state.notificator,
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
    Extension(user): Extension<AccountUser>,
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
        .update_message_body(
            &user,
            form_id,
            answer_id,
            &message_id,
            body_schema.body.map(MessageBody::new),
        )
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
    Extension(user): Extension<AccountUser>,
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
                body: message_with_sender.message.body().as_str().to_owned(),
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
    Extension(user): Extension<AccountUser>,
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
