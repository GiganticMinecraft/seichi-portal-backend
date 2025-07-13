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

use crate::{
    handlers::error_handler::handle_error, schemas::form::form_request_schemas::CommentPostSchema,
};

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

pub async fn update_form_comment(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId, CommentId)>, PathRejection>,
    json: Result<Json<CommentPostSchema>, JsonRejection>,
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
            CommentContent::new(comment_schema.content),
        )
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::OK.into_response())
}

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
