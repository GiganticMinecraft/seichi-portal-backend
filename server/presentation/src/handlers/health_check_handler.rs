use axum::{http::StatusCode, response::IntoResponse};

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200),
    ),
    tag = "Health"
)]
pub async fn health_check() -> impl IntoResponse {
    StatusCode::OK
}
