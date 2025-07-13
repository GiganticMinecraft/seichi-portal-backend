use axum::extract::rejection::PathRejection;
use axum::{
    Extension, Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use domain::form::question::models::Question;
use domain::{form::models::FormId, repository::Repositories, user::models::User};
use errors::ErrorExtra;
use itertools::Itertools;
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::forms::question::QuestionUseCase;

use crate::{
    handlers::error_handler::handle_error,
    schemas::form::form_request_schemas::FormQuestionPutSchema,
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

pub async fn put_question_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormId>, PathRejection>,
    json: Result<Json<FormQuestionPutSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let question_use_case = QuestionUseCase {
        question_repository: repository.form_question_repository(),
    };

    let Path(form_id) = path.map_err_to_error().map_err(handle_error)?;
    let Json(schema) = json.map_err_to_error().map_err(handle_error)?;

    let questions = schema
        .questions
        .iter()
        .map(|question| {
            Question::new(
                question.id,
                form_id,
                question.title.clone(),
                question.description.clone(),
                question.question_type,
                question.choices.clone(),
                question.is_required,
            )
        })
        .collect_vec();

    let questions = question_use_case
        .put_questions(&user, form_id, questions)
        .await
        .map_err(handle_error)?;

    Ok((StatusCode::OK, Json(json!({"questions": questions }))).into_response())
}
