use axum::extract::rejection::JsonRejection;
use axum::response::Response;
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use domain::{
    form::comment::models::{Comment, CommentContent, CommentId},
    repository::Repositories,
    user::models::User,
};
use resource::repository::RealInfrastructureRepository;
use usecase::forms::comment::CommentUseCase;

use crate::{
    handlers::error_handler::{handle_error, handle_json_rejection},
    schemas::form::form_request_schemas::CommentPostSchema,
};

pub async fn post_form_comment(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    json: Result<Json<CommentPostSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_comment_use_case = CommentUseCase {
        comment_repository: repository.form_comment_repository(),
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
    };

    let Json(comment_schema) = json.map_err(handle_json_rejection)?;

    let post_comment_result = async {
        let comment = Comment::new(
            comment_schema.answer_id,
            CommentContent::new(comment_schema.content.try_into()?),
            user.to_owned(),
        );

        form_comment_use_case
            .post_comment(&user, comment, comment_schema.answer_id)
            .await
    }
    .await;

    Ok(match post_comment_result {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => handle_error(err).into_response(),
    })
}

pub async fn delete_form_comment_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path(comment_id): Path<CommentId>,
) -> impl IntoResponse {
    let form_comment_use_case = CommentUseCase {
        comment_repository: repository.form_comment_repository(),
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
    };

    match form_comment_use_case
        .delete_comment(&user, comment_id)
        .await
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}
