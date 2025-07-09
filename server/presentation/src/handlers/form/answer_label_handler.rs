use axum::extract::rejection::PathRejection;
use axum::{
    Extension, Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use domain::{
    form::answer::models::{AnswerId, AnswerLabel, AnswerLabelId},
    repository::Repositories,
    user::models::User,
};
use errors::ErrorExtra;
use resource::repository::RealInfrastructureRepository;
use usecase::forms::answer_label::AnswerLabelUseCase;

use crate::{
    handlers::error_handler::handle_error,
    schemas::form::form_request_schemas::{AnswerLabelSchema, ReplaceAnswerLabelSchema},
};

pub async fn create_label_for_answers(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    json: Result<Json<AnswerLabelSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let answer_label_use_case = AnswerLabelUseCase {
        answer_label_repository: repository.answer_label_repository(),
    };

    let Json(label) = json.map_err_to_error().map_err(handle_error)?;

    answer_label_use_case
        .create_label_for_answers(&user, label.name)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::CREATED.into_response())
}

pub async fn get_labels_for_answers(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
) -> Result<impl IntoResponse, Response> {
    let answer_label_use_case = AnswerLabelUseCase {
        answer_label_repository: repository.answer_label_repository(),
    };

    let labels = answer_label_use_case
        .get_labels_for_answers(&user)
        .await
        .map_err(handle_error)?;
    Ok((StatusCode::OK, Json(labels)).into_response())
}

pub async fn delete_label_for_answers(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<AnswerLabelId>, PathRejection>,
) -> Result<impl IntoResponse, Response> {
    let answer_label_use_case = AnswerLabelUseCase {
        answer_label_repository: repository.answer_label_repository(),
    };

    let Path(label_id) = path.map_err_to_error().map_err(handle_error)?;

    answer_label_use_case
        .delete_label_for_answers(&user, label_id)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::OK.into_response())
}

pub async fn edit_label_for_answers(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<AnswerLabelId>, PathRejection>,
    Json(label): Json<AnswerLabelSchema>,
) -> Result<impl IntoResponse, Response> {
    let answer_label_use_case = AnswerLabelUseCase {
        answer_label_repository: repository.answer_label_repository(),
    };

    let Path(label_id) = path.map_err_to_error().map_err(handle_error)?;

    answer_label_use_case
        .edit_label_for_answers(&user, AnswerLabel::from_raw_parts(label_id, label.name))
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::OK.into_response())
}

pub async fn replace_answer_labels(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<AnswerId>, PathRejection>,
    Json(label_ids): Json<ReplaceAnswerLabelSchema>,
) -> Result<impl IntoResponse, Response> {
    let answer_label_use_case = AnswerLabelUseCase {
        answer_label_repository: repository.answer_label_repository(),
    };

    let Path(answer_id) = path.map_err_to_error().map_err(handle_error)?;

    answer_label_use_case
        .replace_answer_labels(&user, answer_id, label_ids.labels)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::OK.into_response())
}
