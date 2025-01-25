use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
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
                        answer_dto.contents,
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
    Extension(_user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path(_answer_id): Path<AnswerId>,
) -> impl IntoResponse {
    let _form_answer_use_case = AnswerUseCase {
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
        comment_repository: repository.form_comment_repository(),
        answer_label_repository: repository.answer_label_repository(),
        question_repository: repository.form_question_repository(),
    };

    // FIXME: ドメイン知識が handler に紛れ込んでいる
    todo!()

    // let answer_dto = match form_answer_use_case.get_answers(answer_id).await {
    //     Ok(answer) => answer,
    //     Err(err) => return handle_error(err).into_response(),
    // };
    //
    // if user.role == StandardUser {
    //     let forms = match form_answer_use_case
    //         .get_form(&user, answer_dto.form_answer.form_id)
    //         .await
    //     {
    //         Ok(forms) => forms,
    //         Err(err) => return handle_error(err).into_response(),
    //     };
    //
    //     if *forms.settings().answer_visibility() == PRIVATE {
    //         return (
    //             StatusCode::FORBIDDEN,
    //             Json(json!({
    //                 "errorCode": "DO_NOT_HAVE_PERMISSION_TO_GET_ANSWER",
    //                 "reason": "This forms answer visibility is private."
    //             })),
    //         )
    //             .into_response();
    //     }
    // }
    //
    // (
    //     StatusCode::OK,
    //     Json(json!(FormAnswer::new(
    //         answer_dto.form_answer,
    //         answer_dto.contents,
    //         answer_dto.comments,
    //         answer_dto.labels
    //     ))),
    // )
    //     .into_response()
}

pub async fn get_answer_by_form_id_handler(
    Extension(_user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path(_form_id): Path<FormId>,
) -> impl IntoResponse {
    let _form_answer_use_case = AnswerUseCase {
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
        comment_repository: repository.form_comment_repository(),
        answer_label_repository: repository.answer_label_repository(),
        question_repository: repository.form_question_repository(),
    };

    // FIXME: ドメイン知識が handler に紛れ込んでいる
    todo!()
    // if user.role == StandardUser {
    //     match form_answer_use_case.get_form(&user, form_id).await {
    //         Ok(forms) if *forms.settings().answer_visibility() == PRIVATE => {
    //             return (
    //                 StatusCode::FORBIDDEN,
    //                 Json(json!({
    //                     "errorCode": "DO_NOT_HAVE_PERMISSION_TO_GET_ANSWERS",
    //                     "reason": "This forms answer visibility is private."
    //                 })),
    //             )
    //                 .into_response();
    //         }
    //         _ => {}
    //     }
    // }
    //
    // match form_answer_use_case.get_answers_by_form_id(form_id).await {
    //     Ok(answers) => {
    //         let response = answers
    //             .into_iter()
    //             .map(|answer_dto| {
    //                 FormAnswer::new(
    //                     answer_dto.form_answer,
    //                     answer_dto.contents,
    //                     answer_dto.comments,
    //                     answer_dto.labels,
    //                 )
    //             })
    //             .collect_vec();
    //
    //         (StatusCode::OK, Json(response)).into_response()
    //     }
    //     Err(err) => handle_error(err).into_response(),
    // }
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
        .update_answer_meta(answer_id, schema.title)
        .await
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}
