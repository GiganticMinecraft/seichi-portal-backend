use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use domain::{
    form::models::{Form, FormId, OffsetAndLimit},
    repository::Repositories,
};
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::form::FormUseCase;

pub async fn create_form_handler(
    State(repository): State<RealInfrastructureRepository>,
    Json(form): Json<Form>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        ctx: repository.form_repository(),
    };
    match form_use_case.create_form(form.title().to_owned()).await {
        Ok(id) => (StatusCode::CREATED, json!({ "id": id }).to_string()),
        Err(err) => {
            tracing::error!("{}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, "".to_owned())
        }
    }
}

pub async fn form_list_handler(
    State(repository): State<RealInfrastructureRepository>,
    Query(offset_and_limit): Query<OffsetAndLimit>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        ctx: repository.form_repository(),
    };

    match form_use_case
        .form_list(offset_and_limit.offset, offset_and_limit.limit)
        .await
    {
        Ok(forms) => (StatusCode::OK, json!(forms).to_string()),
        Err(err) => {
            tracing::error!("{}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, "".to_owned())
        }
    }
}

pub async fn get_form_handler(
    State(repository): State<RealInfrastructureRepository>,
    Path(form_id): Path<FormId>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        ctx: repository.form_repository(),
    };

    match form_use_case.get_form(form_id).await {
        Ok(form) => (StatusCode::OK, json!(form).to_string()),
        Err(err) => {
            tracing::error!("{}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, "".to_owned())
        }
    }
}
