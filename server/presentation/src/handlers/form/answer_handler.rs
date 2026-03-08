use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::response::Response;
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use domain::form::answer::models::{FormAnswerContent, FormAnswerContentId};
use domain::{
    form::{answer::models::AnswerId, models::FormId},
    repository::Repositories,
    user::models::User,
};
use errors::ErrorExtra;
use itertools::Itertools;
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::forms::answer::AnswerUseCase;

use crate::schemas::error_responses::*;
use crate::{
    handlers::error_handler::handle_error,
    schemas::form::{
        form_request_schemas::{AnswerCreateSchema, AnswerUpdateSchema},
        form_response_schemas::FormAnswer,
    },
};

#[derive(utoipa::IntoResponses)]
pub enum GetAllAnswersResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(Vec<FormAnswer>),
}

impl IntoResponse for GetAllAnswersResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(json!(body))).into_response(),
        }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum GetAnswerResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(FormAnswer),
}

impl IntoResponse for GetAnswerResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(json!(body))).into_response(),
        }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum GetAnswersByFormResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(Vec<FormAnswer>),
}

impl IntoResponse for GetAnswersByFormResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum UpdateAnswerResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(FormAnswer),
}

impl IntoResponse for UpdateAnswerResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

#[utoipa::path(
    get,
    path = "/forms/answers",
    summary = "すべての回答をフォームを横断して取得",
    responses(
        GetAllAnswersResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Answers"
)]
pub async fn get_all_answers(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
) -> Result<GetAllAnswersResponse, Response> {
    let form_answer_use_case = AnswerUseCase {
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
        comment_repository: repository.form_comment_repository(),
        answer_label_repository: repository.answer_label_repository(),
        question_repository: repository.form_question_repository(),
    };

    let answers = form_answer_use_case
        .get_all_answers(&user)
        .await
        .map_err(handle_error)?;

    Ok(GetAllAnswersResponse::Ok(
        answers
            .into_iter()
            .map(|answer_dto| {
                FormAnswer::new(
                    answer_dto.form_answer,
                    answer_dto.comments,
                    answer_dto.labels,
                )
            })
            .collect_vec(),
    ))
}

#[utoipa::path(
    get,
    path = "/forms/{form_id}/answers/{answer_id}",
    summary = "回答の取得",
    params(
        ("form_id" = String, Path, description = "Form ID"),
        ("answer_id" = String, Path, description = "Answer ID"),
    ),
    responses(
        GetAnswerResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Answers"
)]
pub async fn get_answer_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId)>, PathRejection>,
) -> Result<GetAnswerResponse, Response> {
    let form_answer_use_case = AnswerUseCase {
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
        comment_repository: repository.form_comment_repository(),
        answer_label_repository: repository.answer_label_repository(),
        question_repository: repository.form_question_repository(),
    };

    let Path((form_id, answer_id)) = path.map_err_to_error().map_err(handle_error)?;

    let answer_dto = form_answer_use_case
        .get_answers(form_id, answer_id, &user)
        .await
        .map_err(handle_error)?;

    Ok(GetAnswerResponse::Ok(FormAnswer::new(
        answer_dto.form_answer,
        answer_dto.comments,
        answer_dto.labels,
    )))
}

#[utoipa::path(
    get,
    path = "/forms/{id}/answers",
    summary = "回答の一覧取得",
    params(
        ("id" = String, Path, description = "Form ID"),
    ),
    responses(
        GetAnswersByFormResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Answers"
)]
pub async fn get_answer_by_form_id_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormId>, PathRejection>,
) -> Result<GetAnswersByFormResponse, Response> {
    let form_answer_use_case = AnswerUseCase {
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
        comment_repository: repository.form_comment_repository(),
        answer_label_repository: repository.answer_label_repository(),
        question_repository: repository.form_question_repository(),
    };

    let Path(form_id) = path.map_err_to_error().map_err(handle_error)?;

    let answers = form_answer_use_case
        .get_answers_by_form_id(form_id, &user)
        .await
        .map_err(handle_error)?;

    Ok(GetAnswersByFormResponse::Ok(
        answers
            .into_iter()
            .map(|answer_dto| {
                FormAnswer::new(
                    answer_dto.form_answer,
                    answer_dto.comments,
                    answer_dto.labels,
                )
            })
            .collect_vec(),
    ))
}

#[utoipa::path(
    post,
    path = "/forms/{id}/answers",
    summary = "回答の作成",
    params(
        ("id" = String, Path, description = "Form ID"),
    ),
    request_body = AnswerCreateSchema,
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
    tag = "Answers"
)]
pub async fn post_answer_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormId>, PathRejection>,
    json: Result<Json<AnswerCreateSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_answer_use_case = AnswerUseCase {
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
        comment_repository: repository.form_comment_repository(),
        answer_label_repository: repository.answer_label_repository(),
        question_repository: repository.form_question_repository(),
    };

    let Path(form_id) = path.map_err_to_error().map_err(handle_error)?;
    let Json(schema) = json.map_err_to_error().map_err(handle_error)?;

    let answer_contents = schema
        .contents
        .into_iter()
        .map(|schema| FormAnswerContent {
            id: FormAnswerContentId::new(),
            question_id: schema.question_id,
            answer: schema.answer,
        })
        .collect_vec();

    form_answer_use_case
        .post_answers(user, form_id, answer_contents)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::OK.into_response())
}

#[utoipa::path(
    patch,
    path = "/forms/{form_id}/answers/{answer_id}",
    summary = "回答の更新",
    params(
        ("form_id" = String, Path, description = "Form ID"),
        ("answer_id" = String, Path, description = "Answer ID"),
    ),
    request_body = AnswerUpdateSchema,
    responses(
        UpdateAnswerResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Answers"
)]
pub async fn update_answer_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId)>, PathRejection>,
    json: Result<Json<AnswerUpdateSchema>, JsonRejection>,
) -> Result<UpdateAnswerResponse, Response> {
    let form_answer_use_case = AnswerUseCase {
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
        comment_repository: repository.form_comment_repository(),
        answer_label_repository: repository.answer_label_repository(),
        question_repository: repository.form_question_repository(),
    };

    let Path((form_id, answer_id)) = path.map_err_to_error().map_err(handle_error)?;
    let Json(schema) = json.map_err_to_error().map_err(handle_error)?;

    let answer_dto = form_answer_use_case
        .update_answer_meta(form_id, answer_id, &user, schema.title)
        .await
        .map_err(handle_error)?;

    Ok(UpdateAnswerResponse::Ok(FormAnswer::new(
        answer_dto.form_answer,
        answer_dto.comments,
        answer_dto.labels,
    )))
}
