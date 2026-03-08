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
use usecase::forms::question::QuestionUseCase;

use crate::schemas::error_responses::*;
use crate::schemas::form::form_response_schemas::{
    PutQuestionsResponseSchema, QuestionResponseSchema,
};
use crate::{
    handlers::error_handler::handle_error,
    schemas::form::form_request_schemas::FormQuestionPutSchema,
};

#[derive(utoipa::IntoResponses)]
pub enum GetQuestionsResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(Vec<QuestionResponseSchema>),
}

impl IntoResponse for GetQuestionsResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum PutQuestionsResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(PutQuestionsResponseSchema),
}

impl IntoResponse for PutQuestionsResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

#[utoipa::path(
    get,
    path = "/forms/{id}/questions",
    summary = "質問の一覧取得",
    params(
        ("id" = String, Path, description = "Form ID"),
    ),
    responses(
        GetQuestionsResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Questions"
)]
pub async fn get_questions_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormId>, PathRejection>,
) -> Result<GetQuestionsResponse, Response> {
    let question_use_case = QuestionUseCase {
        question_repository: repository.form_question_repository(),
    };

    let Path(form_id) = path.map_err_to_error().map_err(handle_error)?;

    let questions = question_use_case
        .get_questions(&user, form_id)
        .await
        .map_err(handle_error)?;
    Ok(GetQuestionsResponse::Ok(
        questions.into_iter().map(Into::into).collect(),
    ))
}

#[utoipa::path(
    put,
    path = "/forms/{id}/questions",
    summary = "質問の上書き",
    params(
        ("id" = String, Path, description = "Form ID"),
    ),
    request_body = FormQuestionPutSchema,
    responses(
        PutQuestionsResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Questions"
)]
pub async fn put_question_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormId>, PathRejection>,
    json: Result<Json<FormQuestionPutSchema>, JsonRejection>,
) -> Result<PutQuestionsResponse, Response> {
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

    Ok(PutQuestionsResponse::Ok(PutQuestionsResponseSchema {
        questions: questions.into_iter().map(Into::into).collect(),
    }))
}
