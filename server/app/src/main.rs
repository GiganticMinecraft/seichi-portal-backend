use axum::http::header::CONTENT_TYPE;
use axum::http::Method;
use axum::routing::post;
use axum::Router;
use database::connection;
use form::handlers::{create_form_handler, FormHandlers};
use form::infrastructure::fetch_forms;
use migration::MigratorTrait;

use crate::config::HTTP;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tower_http::cors::{Any, CorsLayer};

mod config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _connection = connection::database_connection().await;
    migration::Migrator::up(&_connection, None).await?;

    let handlers = Arc::new(
        FormHandlers::builder()
            .forms(Mutex::new(fetch_forms().await?))
            .build(),
    );

    let router = Router::new()
        .route("/forms", post(create_form_handler))
        .with_state(handlers)
        .layer(
            CorsLayer::new()
                .allow_methods([Method::POST])
                .allow_origin(Any) // todo: allow_originを制限する
                .allow_headers([CONTENT_TYPE]),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], HTTP.port.parse().unwrap()));

    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await
        .expect("Fail to serve.");

    Ok(())
}
