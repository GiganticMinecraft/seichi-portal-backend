use axum::{
    Extension, Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use domain::{form::models::FormId, repository::Repositories, user::models::User};
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::forms::question::QuestionUseCase;

use crate::{
    handlers::error_handler::{handle_error, handle_json_rejection},
    schemas::form::form_request_schemas::FormQuestionUpdateSchema,
};

pub async fn get_questions_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path(form_id): Path<FormId>,
) -> Result<impl IntoResponse, Response> {
    let question_use_case = QuestionUseCase {
        question_repository: repository.form_question_repository(),
    };

    let questions = question_use_case
        .get_questions(&user, form_id)
        .await
        .map_err(handle_error)?;
    Ok((StatusCode::OK, Json(questions)).into_response())
}

pub async fn create_question_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    json: Result<Json<FormQuestionUpdateSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let question_use_case = QuestionUseCase {
        question_repository: repository.form_question_repository(),
    };

    let Json(questions) = json.map_err(handle_json_rejection)?;

    question_use_case
        .create_questions(&user, questions.form_id, questions.questions)
        .await
        .map_err(handle_error)?;

    Ok((StatusCode::CREATED, Json(json!({"id": questions.form_id }))).into_response())
}

pub async fn put_question_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    json: Result<Json<FormQuestionUpdateSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let question_use_case = QuestionUseCase {
        question_repository: repository.form_question_repository(),
    };

    let Json(questions) = json.map_err(handle_json_rejection)?;

    question_use_case
        .put_questions(&user, questions.form_id, questions.questions)
        .await
        .map_err(handle_error)?;

    Ok((StatusCode::OK, Json(json!({"id": questions.form_id }))).into_response())
}
