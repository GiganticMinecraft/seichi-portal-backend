use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use domain::{form::models::FormName, repository::Repositories};
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
        Ok(id) => (StatusCode::CREATED, json!({ "id": id }).to_string()),
        Err(err) => {
            tracing::error!("{}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, "".to_owned())
        }
    }
}
