use axum::{extract::State, http::StatusCode, response::IntoResponse};
use domain::repository::Repositories;
use resource::repository::RealInfrastructureRepository;
use usecase::health_check::HealthCheckUseCase;

pub async fn health_check(
    State(repository): State<RealInfrastructureRepository>,
) -> impl IntoResponse {
    let health_check_use_case = HealthCheckUseCase {
        repository: repository.health_check_repository(),
    };

    if health_check_use_case.health_check().await {
        StatusCode::OK
    } else {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}
