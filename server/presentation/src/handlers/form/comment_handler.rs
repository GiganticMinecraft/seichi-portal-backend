use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
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

    // FIXME: コメントは handler 側で作られるべきではないし、
    //  コメントの id がデータベースで降られるなら Option になるべき。
    let comment = Comment {
        // NOTE: コメントはデータベースで insert した後に id が振られるのでデフォルト値を入れておく
        comment_id: Default::default(),
        answer_id: comment_schema.answer_id,
        content: comment_schema.content,
        timestamp: chrono::Utc::now(),
        commented_by: user.to_owned(),
    };

    match form_comment_use_case
        .post_comment(&user, comment, comment_schema.answer_id)
        .await
    {
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
