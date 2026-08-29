use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::{Extension, Json, extract::Path, extract::State, response::IntoResponse};
use domain::{
    account::models::AccountUser,
    form::answer::{AnswerId, AnswerReference},
    form::models::FormId,
    repository::Repositories,
};
use errors::ErrorExtra;
use resource::repository::{RealInfrastructureRepository, Repository};
use serde_json::json;
use usecase::forms::answer_relation::AnswerRelationUseCase;
use utoipa::IntoResponses;

use crate::{
    handlers::error_handler::{ApiError, handle_error},
    schemas::error_responses::{
        BadRequest, Forbidden, InternalServerError, NotFound, Unauthorized, UnprocessableEntity,
    },
    schemas::form::{
        form_request_schemas::RelatedAnswerRequest, form_response_schemas::RelatedAnswerResponse,
    },
};

type ResourceRepository = Repository<resource::database::connection::ConnectionPool>;
type ResourceAnswerRelationUseCase<'a> = AnswerRelationUseCase<
    'a,
    ResourceRepository,
    ResourceRepository,
    ResourceRepository,
    ResourceRepository,
>;

fn build_answer_relation_use_case<'a>(
    repository: &'a RealInfrastructureRepository,
) -> ResourceAnswerRelationUseCase<'a> {
    AnswerRelationUseCase {
        active_form_repository: repository.active_form_repository(),
        archived_form_repository: repository.archived_form_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        answer_relation_repository: repository.answer_relation_repository(),
    }
}

#[derive(IntoResponses)]
pub enum GetRelatedAnswersResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(Vec<RelatedAnswerResponse>),
}

impl IntoResponse for GetRelatedAnswersResponse {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::Ok(body) => (axum::http::StatusCode::OK, Json(json!(body))).into_response(),
        }
    }
}

#[utoipa::path(
    get,
    path = "/forms/{form_id}/answers/{answer_id}/related-answers",
    summary = "回答に直接関連する回答を取得する",
    params(
        ("form_id" = String, Path, description = "Form ID"),
        ("answer_id" = String, Path, description = "Answer ID"),
    ),
    responses(
        GetRelatedAnswersResponse,
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
pub async fn get_related_answers_handler(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId)>, PathRejection>,
) -> Result<GetRelatedAnswersResponse, ApiError> {
    let Path((form_id, answer_id)) = path.map_err_to_error().map_err(handle_error)?;
    let use_case = build_answer_relation_use_case(&repository);
    let related_answers = use_case
        .list_related_answers(&user, form_id, answer_id)
        .await
        .map_err(handle_error)?
        .into_iter()
        .map(Into::into)
        .collect();

    Ok(GetRelatedAnswersResponse::Ok(related_answers))
}

#[utoipa::path(
    post,
    path = "/forms/{form_id}/answers/{answer_id}/related-answers",
    summary = "回答に別の回答を関連付ける",
    params(
        ("form_id" = String, Path, description = "Form ID"),
        ("answer_id" = String, Path, description = "Answer ID"),
    ),
    request_body = RelatedAnswerRequest,
    responses(
        (status = 200, description = "The relation has been added."),
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
pub async fn add_related_answer_handler(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId)>, PathRejection>,
    json_body: Result<Json<RelatedAnswerRequest>, JsonRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Path((form_id, answer_id)) = path.map_err_to_error().map_err(handle_error)?;
    let Json(request) = json_body.map_err_to_error().map_err(handle_error)?;
    let use_case = build_answer_relation_use_case(&repository);
    use_case
        .add_related_answer(
            &user,
            AnswerReference::new(form_id, answer_id),
            AnswerReference::new(request.form_id, request.answer_id),
        )
        .await
        .map_err(handle_error)?;

    Ok(axum::http::StatusCode::OK.into_response())
}

#[utoipa::path(
    delete,
    path = "/forms/{form_id}/answers/{answer_id}/related-answers/{related_answer_id}",
    summary = "回答間の関連付けを解除する",
    params(
        ("form_id" = String, Path, description = "Form ID"),
        ("answer_id" = String, Path, description = "Answer ID"),
        ("related_answer_id" = String, Path, description = "Related answer ID"),
    ),
    responses(
        (status = 200, description = "The relation has been removed."),
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
pub async fn remove_related_answer_handler(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId, AnswerId)>, PathRejection>,
) -> Result<impl IntoResponse, ApiError> {
    let Path((form_id, answer_id, related_answer_id)) =
        path.map_err_to_error().map_err(handle_error)?;
    let use_case = build_answer_relation_use_case(&repository);
    use_case
        .remove_related_answer(
            &user,
            AnswerReference::new(form_id, answer_id),
            related_answer_id,
        )
        .await
        .map_err(handle_error)?;

    Ok(axum::http::StatusCode::OK.into_response())
}
