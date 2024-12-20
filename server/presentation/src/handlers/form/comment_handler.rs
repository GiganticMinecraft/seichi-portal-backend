use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use domain::form::comment::models::CommentContent;
use domain::{
    form::comment::models::{Comment, CommentId},
    repository::Repositories,
    user::models::User,
};
use resource::repository::RealInfrastructureRepository;
use usecase::forms::comment::CommentUseCase;

use crate::{
    handlers::error_handler::handle_error, schemas::form::form_request_schemas::CommentPostSchema,
};

pub async fn post_form_comment(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Json(comment_schema): Json<CommentPostSchema>,
) -> impl IntoResponse {
    let form_comment_use_case = CommentUseCase {
        comment_repository: repository.form_comment_repository(),
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
    };

    let post_comment_result = async {
        let comment = Comment::new(
            comment_schema.answer_id,
            CommentContent::try_new(comment_schema.content)?,
            user.to_owned(),
        );

        form_comment_use_case
            .post_comment(&user, comment, comment_schema.answer_id)
            .await
    }
    .await;

    match post_comment_result {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn delete_form_comment_handler(
    State(repository): State<RealInfrastructureRepository>,
    Path(comment_id): Path<CommentId>,
) -> impl IntoResponse {
    let form_comment_use_case = CommentUseCase {
        comment_repository: repository.form_comment_repository(),
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
    };

    match form_comment_use_case.delete_comment(comment_id).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}
