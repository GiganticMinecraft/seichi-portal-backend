use std::sync::Arc;

use axum::{Extension, Json, http::StatusCode, response::IntoResponse};
use resource::{
    database::connection::ConnectionPool, messaging::connection::MessagingConnectionPool,
};
use serenity::gateway::ConnectionStage;

pub struct HealthCheckState {
    pub db_conn: Arc<ConnectionPool>,
    pub rabbitmq_conn: Arc<MessagingConnectionPool>,
    pub shard_manager: Arc<serenity::all::ShardManager>,
}

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "All dependencies are healthy."),
        (status = 503, description = "One or more dependencies are unavailable."),
    ),
    tag = "Health"
)]
pub async fn health_check(Extension(state): Extension<Arc<HealthCheckState>>) -> impl IntoResponse {
    let (db_ok, meili_ok, mq_ok, discord_ok) = tokio::join!(
        async { state.db_conn.ping_db().await },
        async { state.db_conn.ping_meilisearch().await },
        async { state.rabbitmq_conn.is_rabbitmq_connected() },
        async {
            let runners = state.shard_manager.runners.lock().await;
            runners
                .values()
                .any(|r| r.stage == ConnectionStage::Connected)
        },
    );

    let all_ok = db_ok && meili_ok && mq_ok && discord_ok;
    let status_code = if all_ok {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };

    let body = Json(serde_json::json!({
        "status": if all_ok { "ok" } else { "error" },
        "db": if db_ok { "ok" } else { "error" },
        "meilisearch": if meili_ok { "ok" } else { "error" },
        "rabbitmq": if mq_ok { "ok" } else { "error" },
        "discord": if discord_ok { "ok" } else { "error" },
    }));

    (status_code, body).into_response()
}
