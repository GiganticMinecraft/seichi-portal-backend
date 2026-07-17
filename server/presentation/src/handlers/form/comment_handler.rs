use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::response::Response;
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use domain::form::answer::AnswerId;
use domain::form::comment::CommentHistoryPagePosition;
use domain::form::models::FormId;
use domain::pagination::{PageLimit, PageRequest};
use domain::{
    account::models::AccountUser,
    form::comment::{CommentContent, CommentId},
    repository::Repositories,
};
use errors::{Error, ErrorExtra, presentation::PresentationError};
use resource::repository::RealInfrastructureRepository;
use usecase::forms::comment::CommentUseCase;

use crate::schemas::error_responses::*;
use crate::schemas::form::form_request_schemas::{CommentUpdateSchema, HistoryListQuery};
use crate::schemas::form::form_response_schemas::{AnswerComment, CommentHistoryPageResponse};
use crate::{
    handlers::error_handler::handle_error, schemas::form::form_request_schemas::CommentPostSchema,
};

#[derive(utoipa::IntoResponses)]
pub enum GetFormCommentResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(Vec<AnswerComment>),
}

#[derive(serde::Deserialize, serde::Serialize)]
struct CommentHistoryCursor {
    after_history_id: uuid::Uuid,
}

fn history_page_request(
    query: HistoryListQuery,
) -> Result<PageRequest<CommentHistoryPagePosition>, Error> {
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
            let cursor: CommentHistoryCursor = serde_json::from_slice(&decoded).map_err(|_| {
                Error::from(PresentationError::QueryRejection {
                    cause: "Invalid cursor.".to_string(),
                })
            })?;
            Ok::<_, Error>(CommentHistoryPagePosition::new(
                cursor.after_history_id.into(),
            ))
        })
        .transpose()?;
    Ok(PageRequest::new(after, limit))
}

fn encode_history_cursor(position: CommentHistoryPagePosition) -> Result<String, Error> {
    let bytes = serde_json::to_vec(&CommentHistoryCursor {
        after_history_id: position.id().into_inner(),
    })
    .map_err(|_| {
        Error::from(PresentationError::QueryRejection {
            cause: "Invalid cursor.".to_string(),
        })
    })?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

#[utoipa::path(
    get,
    path = "/forms/{form_id}/answers/{answer_id}/comments/history",
    summary = "コメントの変更履歴を取得",
    params(("form_id" = String, Path), ("answer_id" = String, Path), HistoryListQuery),
    responses((status = 200, body = CommentHistoryPageResponse), BadRequest, Unauthorized, Forbidden, NotFound, InternalServerError),
    security(("bearer" = [])),
    tag = "Comments"
)]
pub async fn get_comment_history(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId)>, PathRejection>,
    query: Query<HistoryListQuery>,
) -> Result<Json<CommentHistoryPageResponse>, Response> {
    let use_case = CommentUseCase {
        active_form_repository: repository.active_form_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_repository: repository.comment_repository(),
    };
    let Path((form_id, answer_id)) = path.map_err_to_error().map_err(handle_error)?;
    let page = use_case
        .get_history(
            &user,
            form_id,
            answer_id,
            history_page_request(query.0).map_err(handle_error)?,
        )
        .await
        .map_err(handle_error)?;
    let (items, next) = page.into_parts();
    Ok(Json(CommentHistoryPageResponse {
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

impl IntoResponse for GetFormCommentResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

#[utoipa::path(
    get,
    path = "/forms/{form_id}/answers/{answer_id}/comments",
    summary = "コメントの取得",
    params(
        ("form_id" = String, Path, description = "Form ID"),
        ("answer_id" = String, Path, description = "Answer ID"),
    ),
    responses(
        GetFormCommentResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Comments"
)]
pub async fn get_form_comment(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId)>, PathRejection>,
) -> Result<GetFormCommentResponse, Response> {
    let form_comment_use_case = CommentUseCase {
        active_form_repository: repository.active_form_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_repository: repository.comment_repository(),
    };

    let Path((form_id, answer_id)) = path.map_err_to_error().map_err(handle_error)?;

    let comments = form_comment_use_case
        .get_comments(&user, form_id, answer_id)
        .await
        .map_err(handle_error)?;

    Ok(GetFormCommentResponse::Ok(
        comments
            .into_iter()
            .map(Into::<AnswerComment>::into)
            .collect(),
    ))
}

#[utoipa::path(
    post,
    path = "/forms/{form_id}/answers/{answer_id}/comments",
    summary = "コメントの作成",
    params(
        ("form_id" = String, Path, description = "Form ID"),
        ("answer_id" = String, Path, description = "Answer ID"),
    ),
    request_body = CommentPostSchema,
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
    tag = "Comments"
)]
pub async fn post_form_comment(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId)>, PathRejection>,
    json: Result<Json<CommentPostSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_comment_use_case = CommentUseCase {
        active_form_repository: repository.active_form_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_repository: repository.comment_repository(),
    };

    let Path((form_id, answer_id)) = path.map_err_to_error().map_err(handle_error)?;
    let Json(comment_schema) = json.map_err_to_error().map_err(handle_error)?;

    form_comment_use_case
        .post_comment(
            &user,
            form_id,
            answer_id,
            CommentContent::new(comment_schema.content),
        )
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::OK.into_response())
}

#[utoipa::path(
    patch,
    path = "/forms/{form_id}/answers/{answer_id}/comments/{comment_id}",
    summary = "コメントの編集",
    params(
        ("form_id" = String, Path, description = "Form ID"),
        ("answer_id" = String, Path, description = "Answer ID"),
        ("comment_id" = String, Path, description = "Comment ID"),
    ),
    request_body = CommentUpdateSchema,
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
    tag = "Comments"
)]
pub async fn update_form_comment(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId, CommentId)>, PathRejection>,
    json: Result<Json<CommentUpdateSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_comment_use_case = CommentUseCase {
        active_form_repository: repository.active_form_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_repository: repository.comment_repository(),
    };

    let Path((form_id, answer_id, comment_id)) = path.map_err_to_error().map_err(handle_error)?;
    let Json(comment_schema) = json.map_err_to_error().map_err(handle_error)?;

    form_comment_use_case
        .update_comment(
            &user,
            form_id,
            answer_id,
            comment_id,
            comment_schema.content.map(CommentContent::new),
        )
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::OK.into_response())
}

#[utoipa::path(
    delete,
    path = "/forms/{form_id}/answers/{answer_id}/comments/{comment_id}",
    summary = "コメントの削除",
    params(
        ("form_id" = String, Path, description = "Form ID"),
        ("answer_id" = String, Path, description = "Answer ID"),
        ("comment_id" = String, Path, description = "Comment ID"),
    ),
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
    tag = "Comments"
)]
pub async fn delete_form_comment_handler(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId, CommentId)>, PathRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_comment_use_case = CommentUseCase {
        active_form_repository: repository.active_form_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_repository: repository.comment_repository(),
    };

    let Path((form_id, answer_id, comment_id)) = path.map_err_to_error().map_err(handle_error)?;

    form_comment_use_case
        .delete_comment(&user, form_id, answer_id, comment_id)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::OK.into_response())
}
