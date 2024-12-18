use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use domain::{form::models::FormId, repository::Repositories};
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::forms::question::QuestionUseCase;

use crate::{
    handlers::error_handler::handle_error,
    schemas::form::form_request_schemas::FormQuestionUpdateSchema,
};

pub async fn get_questions_handler(
    State(repository): State<RealInfrastructureRepository>,
    Path(form_id): Path<FormId>,
) -> impl IntoResponse {
    let question_use_case = QuestionUseCase {
        question_repository: repository.form_question_repository(),
    };

    match question_use_case.get_questions(form_id).await {
        Ok(questions) => (StatusCode::OK, Json(questions)).into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn create_question_handler(
    State(repository): State<RealInfrastructureRepository>,
    Json(questions): Json<FormQuestionUpdateSchema>,
) -> impl IntoResponse {
    let question_use_case = QuestionUseCase {
        question_repository: repository.form_question_repository(),
    };

    match question_use_case
        .create_questions(questions.form_id, questions.questions)
        .await
    {
        Ok(_) => (StatusCode::CREATED, Json(json!({"id": questions.form_id }))).into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn put_question_handler(
    State(repository): State<RealInfrastructureRepository>,
    Json(questions): Json<FormQuestionUpdateSchema>,
) -> impl IntoResponse {
    let question_use_case = QuestionUseCase {
        question_repository: repository.form_question_repository(),
    };

    match question_use_case
        .put_questions(questions.form_id, questions.questions)
        .await
    {
        Ok(_) => (StatusCode::OK, Json(json!({"id": questions.form_id }))).into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}
