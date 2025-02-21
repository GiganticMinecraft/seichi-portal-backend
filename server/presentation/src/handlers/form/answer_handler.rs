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
) -> impl IntoResponse {
    let form_answer_use_case = AnswerUseCase {
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
        comment_repository: repository.form_comment_repository(),
        answer_label_repository: repository.answer_label_repository(),
        question_repository: repository.form_question_repository(),
    };
    match form_answer_use_case.get_all_answers(&user).await {
        Ok(answers) => {
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

            (StatusCode::OK, Json(json!(response))).into_response()
        }
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn get_answer_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path(answer_id): Path<AnswerId>,
) -> impl IntoResponse {
    let form_answer_use_case = AnswerUseCase {
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
        comment_repository: repository.form_comment_repository(),
        answer_label_repository: repository.answer_label_repository(),
        question_repository: repository.form_question_repository(),
    };

    let answer_dto = match form_answer_use_case.get_answers(answer_id, &user).await {
        Ok(answer) => answer,
        Err(err) => return handle_error(err).into_response(),
    };

    (
        StatusCode::OK,
        Json(json!(FormAnswer::new(
            answer_dto.form_answer,
            answer_dto.comments,
            answer_dto.labels
        ))),
    )
        .into_response()
}

pub async fn get_answer_by_form_id_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path(form_id): Path<FormId>,
) -> impl IntoResponse {
    let form_answer_use_case = AnswerUseCase {
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
        comment_repository: repository.form_comment_repository(),
        answer_label_repository: repository.answer_label_repository(),
        question_repository: repository.form_question_repository(),
    };

    match form_answer_use_case
        .get_answers_by_form_id(form_id, &user)
        .await
    {
        Ok(answers) => {
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

            (StatusCode::OK, Json(response)).into_response()
        }
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn post_answer_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Json(schema): Json<AnswersPostSchema>,
) -> impl IntoResponse {
    let form_answer_use_case = AnswerUseCase {
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
        comment_repository: repository.form_comment_repository(),
        answer_label_repository: repository.answer_label_repository(),
        question_repository: repository.form_question_repository(),
    };
    match form_answer_use_case
        .post_answers(user, schema.form_id, schema.answers)
        .await
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn update_answer_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path(answer_id): Path<AnswerId>,
    Json(schema): Json<AnswerUpdateSchema>,
) -> impl IntoResponse {
    let form_answer_use_case = AnswerUseCase {
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
        comment_repository: repository.form_comment_repository(),
        answer_label_repository: repository.answer_label_repository(),
        question_repository: repository.form_question_repository(),
    };

    match form_answer_use_case
        .update_answer_meta(answer_id, &user, schema.title)
        .await
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}
