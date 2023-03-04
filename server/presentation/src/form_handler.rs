use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use domain::form::models::{FormId, FormName};
use domain::repository::Repositories;
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::create_form::FormUseCase;

pub async fn create_form_handler(
    State(repository): State<RealInfrastructureRepository>,
    Json(form_name): Json<FormName>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        ctx: repository.form_repository(),
    };
    match form_use_case.create_form(form_name).await {
        Ok(FormId(id)) => (StatusCode::CREATED, json!(id).to_string()),
        Err(err) => {
            tracing::error!("{}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, "".to_owned())
        }
    }
}
