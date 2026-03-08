use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::response::Response;
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use domain::form::answer::models::AnswerId;
use domain::form::models::FormId;
use domain::{
    form::comment::models::{Comment, CommentContent, CommentId},
    repository::Repositories,
    user::models::User,
};
use errors::ErrorExtra;
use resource::repository::RealInfrastructureRepository;
use usecase::forms::comment::CommentUseCase;

use crate::schemas::error_responses::*;
use crate::schemas::form::form_request_schemas::CommentUpdateSchema;
use crate::schemas::form::form_response_schemas::AnswerComment;
use crate::{
    handlers::error_handler::handle_error, schemas::form::form_request_schemas::CommentPostSchema,
};

#[derive(utoipa::IntoResponses)]
pub enum GetFormCommentResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(Vec<AnswerComment>),
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
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId)>, PathRejection>,
) -> Result<GetFormCommentResponse, Response> {
    let form_comment_use_case = CommentUseCase {
        comment_repository: repository.form_comment_repository(),
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
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
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId)>, PathRejection>,
    json: Result<Json<CommentPostSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_comment_use_case = CommentUseCase {
        comment_repository: repository.form_comment_repository(),
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
    };

    let Path((form_id, answer_id)) = path.map_err_to_error().map_err(handle_error)?;
    let Json(comment_schema) = json.map_err_to_error().map_err(handle_error)?;

    let comment = Comment::new(
        answer_id,
        CommentContent::new(comment_schema.content),
        user.to_owned(),
    );

    form_comment_use_case
        .post_comment(&user, form_id, answer_id, comment)
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
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId, CommentId)>, PathRejection>,
    json: Result<Json<CommentUpdateSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_comment_use_case = CommentUseCase {
        comment_repository: repository.form_comment_repository(),
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
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
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId, CommentId)>, PathRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_comment_use_case = CommentUseCase {
        comment_repository: repository.form_comment_repository(),
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
    };

    let Path((form_id, answer_id, comment_id)) = path.map_err_to_error().map_err(handle_error)?;

    form_comment_use_case
        .delete_comment(&user, form_id, answer_id, comment_id)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::OK.into_response())
}
