use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use domain::repository::Repositories;
use resource::repository::RealInfrastructureRepository;
use serde_json::{Map, json};
use usecase::health_check::{HealthCheckUseCase, HealthStatus};

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "MariaDB and Valkey are healthy. Optional dependency failures are reported with status 'degraded'."),
        (status = 503, description = "A required dependency is unavailable; the response status is 'error'."),
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
    let health_status = result.status();

    let status_code = if health_status.is_ready() {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let component_map: Map<_, _> = std::iter::once((
        "status".to_string(),
        json!(match health_status {
            HealthStatus::Ok => "ok",
            HealthStatus::Degraded => "degraded",
            HealthStatus::Error => "error",
        }),
    ))
    .chain(
        result
            .components
            .into_iter()
            .map(|c| (c.name, json!(if c.healthy { "ok" } else { "error" }))),
    )
    .collect();

    let body = Json(json!(component_map));

    (status_code, body).into_response()
}
