use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use domain::{
    form::{
        answer::models::AnswerId,
        models::{Label, LabelId},
    },
    repository::Repositories,
};
use resource::repository::RealInfrastructureRepository;
use usecase::forms::answer_label::AnswerLabelUseCase;

use crate::{
    handlers::error_handler::handle_error,
    schemas::form::form_request_schemas::{LabelSchema, ReplaceAnswerLabelSchema},
};

pub async fn create_label_for_answers(
    State(repository): State<RealInfrastructureRepository>,
    Json(label): Json<LabelSchema>,
) -> impl IntoResponse {
    let answer_label_use_case = AnswerLabelUseCase {
        answer_label_repository: repository.answer_label_repository(),
    };

    match answer_label_use_case
        .create_label_for_answers(label.name)
        .await
    {
        Ok(_) => StatusCode::CREATED.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn get_labels_for_answers(
    State(repository): State<RealInfrastructureRepository>,
) -> impl IntoResponse {
    let answer_label_use_case = AnswerLabelUseCase {
        answer_label_repository: repository.answer_label_repository(),
    };

    match answer_label_use_case.get_labels_for_answers().await {
        Ok(labels) => (StatusCode::OK, Json(labels)).into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn delete_label_for_answers(
    State(repository): State<RealInfrastructureRepository>,
    Path(label_id): Path<LabelId>,
) -> impl IntoResponse {
    let answer_label_use_case = AnswerLabelUseCase {
        answer_label_repository: repository.answer_label_repository(),
    };

    match answer_label_use_case
        .delete_label_for_answers(label_id)
        .await
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn edit_label_for_answers(
    State(repository): State<RealInfrastructureRepository>,
    Path(label_id): Path<LabelId>,
    Json(label): Json<LabelSchema>,
) -> impl IntoResponse {
    let answer_label_use_case = AnswerLabelUseCase {
        answer_label_repository: repository.answer_label_repository(),
    };

    match answer_label_use_case
        .edit_label_for_answers(&Label {
            id: label_id,
            name: label.name,
        })
        .await
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn replace_answer_labels(
    State(repository): State<RealInfrastructureRepository>,
    Path(answer_id): Path<AnswerId>,
    Json(label_ids): Json<ReplaceAnswerLabelSchema>,
) -> impl IntoResponse {
    let answer_label_use_case = AnswerLabelUseCase {
        answer_label_repository: repository.answer_label_repository(),
    };

    match answer_label_use_case
        .replace_answer_labels(answer_id, label_ids.labels)
        .await
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}
