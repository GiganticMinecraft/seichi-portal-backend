use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use domain::repository::Repositories;
use resource::repository::RealInfrastructureRepository;
use usecase::health_check::HealthCheckUseCase;

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "All dependencies are healthy."),
        (status = 503, description = "One or more dependencies are unavailable."),
    ),
    tag = "Health"
)]
pub async fn health_check(
    State(repository): State<RealInfrastructureRepository>,
) -> impl IntoResponse {
    let usecase = HealthCheckUseCase {
        repository: repository.health_check_repository(),
    };
    let result = usecase.check().await;
    let all_ok = result.all_ok();

    let status_code = if all_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let component_map: serde_json::Map<_, _> = std::iter::once((
        "status".to_string(),
        serde_json::json!(if all_ok { "ok" } else { "error" }),
    ))
    .chain(result.components.iter().map(|c| {
        (
            c.name.clone(),
            serde_json::json!(if c.healthy { "ok" } else { "error" }),
        )
    }))
    .collect();
    let body = Json(serde_json::Value::Object(component_map));

    (status_code, body).into_response()
}
