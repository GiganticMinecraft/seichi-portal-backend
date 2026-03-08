use axum::extract::rejection::PathRejection;
use axum::http::{HeaderValue, header};
use axum::{
    Extension, Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use domain::{
    form::answer::models::{AnswerId, AnswerLabelId},
    repository::Repositories,
    user::models::User,
};
use errors::ErrorExtra;
use resource::repository::RealInfrastructureRepository;
use usecase::forms::answer_label::AnswerLabelUseCase;

use crate::schemas::error_responses::*;
use crate::schemas::form::form_request_schemas::AnswerLabelSchema;
use crate::schemas::form::form_response_schemas::AnswerLabelResponseSchema;
use crate::{
    handlers::error_handler::handle_error,
    schemas::form::form_request_schemas::{AnswerLabelUpdateSchema, ReplaceAnswerLabelSchema},
};

#[derive(utoipa::IntoResponses)]
pub enum CreateAnswerLabelResponse {
    #[response(
        status = 201,
        description = "The request has succeeded and a new resource has been created as a result."
    )]
    Created(AnswerLabelResponseSchema),
}

impl IntoResponse for CreateAnswerLabelResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Created(body) => (
                StatusCode::CREATED,
                [(
                    header::LOCATION,
                    HeaderValue::from_str(body.id.as_str()).unwrap(),
                )],
                Json(body),
            )
                .into_response(),
        }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum GetAnswerLabelsResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(Vec<AnswerLabelResponseSchema>),
}

impl IntoResponse for GetAnswerLabelsResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum EditAnswerLabelResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(AnswerLabelResponseSchema),
}

impl IntoResponse for EditAnswerLabelResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

#[utoipa::path(
    post,
    path = "/labels/answers",
    summary = "回答用ラベルを作成する",
    request_body = AnswerLabelSchema,
    responses(
        CreateAnswerLabelResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Labels"
)]
pub async fn create_label_for_answers(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    json: Result<Json<AnswerLabelSchema>, JsonRejection>,
) -> Result<CreateAnswerLabelResponse, Response> {
    let answer_label_use_case = AnswerLabelUseCase {
        answer_label_repository: repository.answer_label_repository(),
    };

    let Json(label) = json.map_err_to_error().map_err(handle_error)?;

    let label = answer_label_use_case
        .create_label_for_answers(&user, label.name)
        .await
        .map_err(handle_error)?;

    Ok(CreateAnswerLabelResponse::Created(label.into()))
}

#[utoipa::path(
    get,
    path = "/labels/answers",
    summary = "回答用ラベルの一覧を取得する",
    responses(
        GetAnswerLabelsResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Labels"
)]
pub async fn get_labels_for_answers(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
) -> Result<GetAnswerLabelsResponse, Response> {
    let answer_label_use_case = AnswerLabelUseCase {
        answer_label_repository: repository.answer_label_repository(),
    };

    let labels = answer_label_use_case
        .get_labels_for_answers(&user)
        .await
        .map_err(handle_error)?;
    Ok(GetAnswerLabelsResponse::Ok(
        labels.into_iter().map(Into::into).collect(),
    ))
}

#[utoipa::path(
    delete,
    path = "/labels/answers/{label_id}",
    summary = "回答用ラベルを削除する",
    params(
        ("label_id" = String, Path, description = "Label ID"),
    ),
    responses(
        (status = 200, description = "The request has succeeded."),
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Labels"
)]
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

#[utoipa::path(
    patch,
    path = "/labels/answers/{label_id}",
    summary = "回答用ラベルを更新する",
    params(
        ("label_id" = String, Path, description = "Label ID"),
    ),
    request_body = AnswerLabelUpdateSchema,
    responses(
        EditAnswerLabelResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Labels"
)]
pub async fn edit_label_for_answers(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<AnswerLabelId>, PathRejection>,
    json: Result<Json<AnswerLabelUpdateSchema>, JsonRejection>,
) -> Result<EditAnswerLabelResponse, Response> {
    let answer_label_use_case = AnswerLabelUseCase {
        answer_label_repository: repository.answer_label_repository(),
    };

    let Path(label_id) = path.map_err_to_error().map_err(handle_error)?;
    let Json(label) = json.map_err_to_error().map_err(handle_error)?;

    let updated_label = answer_label_use_case
        .edit_label_for_answers(&user, label_id, label.name)
        .await
        .map_err(handle_error)?;

    Ok(EditAnswerLabelResponse::Ok(updated_label.into()))
}

#[utoipa::path(
    put,
    path = "/forms/answers/{answer_id}/labels",
    params(
        ("answer_id" = String, Path, description = "Answer ID"),
    ),
    request_body = ReplaceAnswerLabelSchema,
    responses(
        (status = 200, description = "The request has succeeded."),
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Labels"
)]
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
