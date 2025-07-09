use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::response::Response;
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
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

use crate::{
    handlers::error_handler::handle_error,
    schemas::form::{
        form_request_schemas::{AnswerUpdateSchema, AnswersPostSchema},
        form_response_schemas::FormAnswer,
    },
};

pub async fn get_all_answers(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
) -> Result<impl IntoResponse, Response> {
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

    let response = answers
        .into_iter()
        .map(|answer_dto| {
            FormAnswer::new(
                answer_dto.form_answer,
                answer_dto.comments,
                answer_dto.labels,
            )
        })
        .collect_vec();

    Ok((StatusCode::OK, Json(json!(response))).into_response())
}

pub async fn get_answer_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<AnswerId>, PathRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_answer_use_case = AnswerUseCase {
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
        comment_repository: repository.form_comment_repository(),
        answer_label_repository: repository.answer_label_repository(),
        question_repository: repository.form_question_repository(),
    };

    let Path(answer_id) = path.map_err_to_error().map_err(handle_error)?;

    let answer_dto = form_answer_use_case
        .get_answers(answer_id, &user)
        .await
        .map_err(handle_error)?;

    Ok((
        StatusCode::OK,
        Json(json!(FormAnswer::new(
            answer_dto.form_answer,
            answer_dto.comments,
            answer_dto.labels
        ))),
    )
        .into_response())
}

pub async fn get_answer_by_form_id_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormId>, PathRejection>,
) -> Result<impl IntoResponse, Response> {
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

    let response = answers
        .into_iter()
        .map(|answer_dto| {
            FormAnswer::new(
                answer_dto.form_answer,
                answer_dto.comments,
                answer_dto.labels,
            )
        })
        .collect_vec();

    Ok((StatusCode::OK, Json(response)).into_response())
}

pub async fn post_answer_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    json: Result<Json<AnswersPostSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_answer_use_case = AnswerUseCase {
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
        comment_repository: repository.form_comment_repository(),
        answer_label_repository: repository.answer_label_repository(),
        question_repository: repository.form_question_repository(),
    };

    let Json(schema) = json.map_err_to_error().map_err(handle_error)?;

    form_answer_use_case
        .post_answers(user, schema.form_id, schema.answers)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::OK.into_response())
}

pub async fn update_answer_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path(answer_id): Path<AnswerId>,
    json: Result<Json<AnswerUpdateSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_answer_use_case = AnswerUseCase {
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
        comment_repository: repository.form_comment_repository(),
        answer_label_repository: repository.answer_label_repository(),
        question_repository: repository.form_question_repository(),
    };

    let Json(schema) = json.map_err_to_error().map_err(handle_error)?;

    form_answer_use_case
        .update_answer_meta(answer_id, &user, schema.title)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::OK.into_response())
}
