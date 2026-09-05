use axum::extract::{
    Multipart,
    multipart::{MultipartError, MultipartRejection as AxumMultipartRejection},
    rejection::{JsonRejection, PathRejection},
};
use axum::response::Response;
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::IntoResponse,
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use domain::form::answer::AnswerId;
use domain::form::comment::CommentHistoryPagePosition;
use domain::form::models::FormId;
use domain::pagination::{PageLimit, PageRequest};
use domain::{
    account::models::AccountUser,
    form::{
        comment::{CommentContent, CommentId},
        comment_attachment::{
            CommentAttachmentId, MAX_COMMENT_ATTACHMENT_SIZE, MAX_COMMENT_ATTACHMENTS_PER_COMMENT,
        },
    },
    repository::Repositories,
};
use errors::{Error, ErrorExtra, presentation::PresentationError};
use resource::repository::RealInfrastructureRepository;
use usecase::forms::comment::{CommentAttachmentUpload, CommentUseCase};

use crate::api::global_discord_webhook::APPLICATION_EVENT_PUBLISHER;
use crate::schemas::error_responses::*;
use crate::schemas::form::form_request_schemas::{CommentUpdateSchema, HistoryListQuery};
use crate::schemas::form::form_response_schemas::{AnswerComment, CommentHistoryPageResponse};
use crate::{
    handlers::error_handler::{ApiError, handle_error},
    schemas::form::form_request_schemas::CommentPostSchema,
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
) -> Result<Json<CommentHistoryPageResponse>, ApiError> {
    let use_case = CommentUseCase {
        active_form_repository: repository.active_form_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_thread_repository: repository.comment_thread_repository(),
        comment_attachment_repository: repository.comment_attachment_repository(),
        application_event_publisher: Some(&APPLICATION_EVENT_PUBLISHER),
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
) -> Result<GetFormCommentResponse, ApiError> {
    let form_comment_use_case = CommentUseCase {
        active_form_repository: repository.active_form_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_thread_repository: repository.comment_thread_repository(),
        comment_attachment_repository: repository.comment_attachment_repository(),
        application_event_publisher: Some(&APPLICATION_EVENT_PUBLISHER),
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
) -> Result<impl IntoResponse, ApiError> {
    let form_comment_use_case = CommentUseCase {
        active_form_repository: repository.active_form_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_thread_repository: repository.comment_thread_repository(),
        comment_attachment_repository: repository.comment_attachment_repository(),
        application_event_publisher: Some(&APPLICATION_EVENT_PUBLISHER),
    };

    let Path((form_id, answer_id)) = path.map_err_to_error().map_err(handle_error)?;
    let Json(comment_schema) = json.map_err_to_error().map_err(handle_error)?;

    form_comment_use_case
        .post_comment(
            &user,
            form_id,
            answer_id,
            CommentContent::new(comment_schema.content),
            repository.form_submission_restriction_repository(),
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
) -> Result<impl IntoResponse, ApiError> {
    let form_comment_use_case = CommentUseCase {
        active_form_repository: repository.active_form_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_thread_repository: repository.comment_thread_repository(),
        comment_attachment_repository: repository.comment_attachment_repository(),
        application_event_publisher: Some(&APPLICATION_EVENT_PUBLISHER),
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
) -> Result<impl IntoResponse, ApiError> {
    let form_comment_use_case = CommentUseCase {
        active_form_repository: repository.active_form_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_thread_repository: repository.comment_thread_repository(),
        comment_attachment_repository: repository.comment_attachment_repository(),
        application_event_publisher: Some(&APPLICATION_EVENT_PUBLISHER),
    };

    let Path((form_id, answer_id, comment_id)) = path.map_err_to_error().map_err(handle_error)?;

    form_comment_use_case
        .delete_comment(&user, form_id, answer_id, comment_id)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::OK.into_response())
}

pub const MAX_COMMENT_ATTACHMENT_REQUEST_SIZE: usize =
    (MAX_COMMENT_ATTACHMENT_SIZE as usize * MAX_COMMENT_ATTACHMENTS_PER_COMMENT) + (1024 * 1024);

fn multipart_error(message: impl Into<String>) -> Error {
    PresentationError::MultipartRejection {
        cause: message.into(),
    }
    .into()
}

fn multipart_parse_error(error: MultipartError) -> Error {
    if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        PresentationError::PayloadTooLarge {
            cause: error.body_text(),
        }
        .into()
    } else {
        multipart_error(error.body_text())
    }
}

async fn parse_comment_attachment_uploads(
    mut multipart: Multipart,
) -> Result<Vec<CommentAttachmentUpload>, Error> {
    let mut uploads = Vec::new();
    let mut total_size = 0usize;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(multipart_parse_error)?
    {
        if field.name() != Some("file") {
            return Err(multipart_error("multipart field must be named file"));
        }
        if uploads.len() >= MAX_COMMENT_ATTACHMENTS_PER_COMMENT {
            return Err(multipart_error(format!(
                "a comment must not have more than {MAX_COMMENT_ATTACHMENTS_PER_COMMENT} attachments"
            )));
        }
        let file_name = field
            .file_name()
            .map(str::to_owned)
            .ok_or_else(|| multipart_error("multipart file name is missing"))?;
        let content_type = field
            .content_type()
            .filter(|content_type| !content_type.is_empty())
            .unwrap_or("application/octet-stream")
            .to_owned();
        let mut content = Vec::new();
        while let Some(chunk) = field.chunk().await.map_err(multipart_parse_error)? {
            if chunk.len() > MAX_COMMENT_ATTACHMENT_SIZE as usize - content.len() {
                return Err(multipart_error(format!(
                    "comment attachment must not exceed {MAX_COMMENT_ATTACHMENT_SIZE} bytes"
                )));
            }
            if chunk.len() > MAX_COMMENT_ATTACHMENT_REQUEST_SIZE - total_size {
                return Err(multipart_error("multipart request is too large"));
            }
            content.extend_from_slice(&chunk);
            total_size += chunk.len();
        }
        uploads.push(CommentAttachmentUpload {
            file_name,
            content_type,
            content,
        });
    }
    if uploads.is_empty() {
        return Err(multipart_error("at least one file is required"));
    }
    Ok(uploads)
}

fn content_disposition(file_name: &str) -> String {
    let encoded = file_name.bytes().fold(String::new(), |mut result, byte| {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            result.push(byte as char);
        } else {
            result.push('%');
            result.push(char::from(b"0123456789ABCDEF"[(byte >> 4) as usize]));
            result.push(char::from(b"0123456789ABCDEF"[(byte & 0x0f) as usize]));
        }
        result
    });
    format!("attachment; filename=download; filename*=UTF-8''{encoded}")
}

#[utoipa::path(
    post,
    path = "/forms/{form_id}/answers/{answer_id}/comments/{comment_id}/attachments",
    summary = "コメントへのファイル添付",
    params(
        ("form_id" = String, Path, description = "Form ID"),
        ("answer_id" = String, Path, description = "Answer ID"),
        ("comment_id" = String, Path, description = "Comment ID"),
    ),
    request_body(
        description = "One or more file fields named file.",
        content_type = "multipart/form-data"
    ),
    responses(
        (status = 200, description = "The request has succeeded."),
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        PayloadTooLarge,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Comments"
)]
pub async fn post_comment_attachments(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId, CommentId)>, PathRejection>,
    multipart: Result<Multipart, AxumMultipartRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let use_case = CommentUseCase {
        active_form_repository: repository.active_form_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_thread_repository: repository.comment_thread_repository(),
        comment_attachment_repository: repository.comment_attachment_repository(),
        application_event_publisher: Some(&APPLICATION_EVENT_PUBLISHER),
    };
    let Path((form_id, answer_id, comment_id)) = path.map_err_to_error().map_err(handle_error)?;
    let multipart =
        multipart.map_err(|error| handle_error(Error::from(PresentationError::from(error))))?;
    let uploads = parse_comment_attachment_uploads(multipart)
        .await
        .map_err(handle_error)?;
    use_case
        .post_attachments(&user, form_id, answer_id, comment_id, uploads)
        .await
        .map_err(handle_error)?;
    Ok(StatusCode::OK.into_response())
}

#[utoipa::path(
    get,
    path = "/forms/{form_id}/answers/{answer_id}/comments/attachments/{attachment_id}",
    summary = "コメント添付ファイルの取得",
    params(
        ("form_id" = String, Path, description = "Form ID"),
        ("answer_id" = String, Path, description = "Answer ID"),
        ("attachment_id" = String, Path, description = "Attachment ID"),
    ),
    responses(
        (status = 200, description = "The requested file."),
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Comments"
)]
pub async fn get_comment_attachment(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId, CommentAttachmentId)>, PathRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let use_case = CommentUseCase {
        active_form_repository: repository.active_form_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_thread_repository: repository.comment_thread_repository(),
        comment_attachment_repository: repository.comment_attachment_repository(),
        application_event_publisher: Some(&APPLICATION_EVENT_PUBLISHER),
    };
    let Path((form_id, answer_id, attachment_id)) =
        path.map_err_to_error().map_err(handle_error)?;
    let (attachment, content) = use_case
        .download_attachment(&user, form_id, answer_id, attachment_id)
        .await
        .map_err(handle_error)?;
    let mut headers = HeaderMap::new();
    let content_type = (!attachment.content_type().is_empty())
        .then_some(attachment.content_type())
        .and_then(|content_type| HeaderValue::from_str(content_type).ok())
        .unwrap_or_else(|| HeaderValue::from_static("application/octet-stream"));
    headers.insert(header::CONTENT_TYPE, content_type);
    headers.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&content_disposition(attachment.file_name().as_str()))
            .expect("content disposition only contains ASCII characters"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    Ok((headers, content).into_response())
}

#[utoipa::path(
    delete,
    path = "/forms/{form_id}/answers/{answer_id}/comments/attachments/{attachment_id}",
    summary = "コメント添付ファイルの削除",
    params(
        ("form_id" = String, Path, description = "Form ID"),
        ("answer_id" = String, Path, description = "Answer ID"),
        ("attachment_id" = String, Path, description = "Attachment ID"),
    ),
    responses(
        (status = 200, description = "The request has succeeded."),
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Comments"
)]
pub async fn delete_comment_attachment(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId, CommentAttachmentId)>, PathRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let use_case = CommentUseCase {
        active_form_repository: repository.active_form_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_thread_repository: repository.comment_thread_repository(),
        comment_attachment_repository: repository.comment_attachment_repository(),
        application_event_publisher: Some(&APPLICATION_EVENT_PUBLISHER),
    };
    let Path((form_id, answer_id, attachment_id)) =
        path.map_err_to_error().map_err(handle_error)?;
    use_case
        .delete_attachment(&user, form_id, answer_id, attachment_id)
        .await
        .map_err(handle_error)?;
    Ok(StatusCode::OK.into_response())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        extract::FromRequest,
        http::{Request, header::CONTENT_TYPE},
    };

    #[tokio::test]
    async fn parses_repeated_file_fields_without_trusting_client_size() {
        let request = Request::builder()
            .header(
                CONTENT_TYPE,
                "multipart/form-data; boundary=attachment-boundary",
            )
            .body(Body::from(
                "--attachment-boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"first.txt\"\r\nContent-Type: text/plain\r\n\r\nfirst\r\n--attachment-boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"second.txt\"\r\n\r\nsecond\r\n--attachment-boundary--\r\n",
            ))
            .unwrap();
        let multipart = Multipart::from_request(request, &()).await.unwrap();

        let uploads = parse_comment_attachment_uploads(multipart).await.unwrap();

        assert_eq!(uploads.len(), 2);
        assert_eq!(uploads[0].file_name, "first.txt");
        assert_eq!(uploads[0].content, b"first");
        assert_eq!(uploads[1].content_type, "application/octet-stream");
    }

    #[test]
    fn content_disposition_percent_encodes_the_original_file_name() {
        assert_eq!(
            content_disposition("レポート.txt"),
            "attachment; filename=download; filename*=UTF-8''%E3%83%AC%E3%83%9D%E3%83%BC%E3%83%88.txt"
        );
    }
}
