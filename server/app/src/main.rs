use axum::http::header::CONTENT_TYPE;
use axum::http::Method;
use axum::routing::post;
use axum::Router;
use database::connection;
use form::handlers::{create_form_handler, FormHandlers};
use form::infrastructure::fetch_forms;
use migration::MigratorTrait;

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tower_http::cors::{Any, CorsLayer};

#[tokio::main]
async fn main() {
    let _connection = connection::database_connection().await;
    migration::Migrator::up(&_connection, None).await.unwrap();

    let handlers = Arc::new(
        FormHandlers::builder()
            .forms(Mutex::new(fetch_forms().await))
            .build(),
    );

    let router = Router::new()
        .route("/api/forms", post(create_form_handler))
        .with_state(handlers)
        .layer(
            CorsLayer::new()
                .allow_methods([Method::POST])
                .allow_origin(Any)
                .allow_headers([CONTENT_TYPE]),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], 9000));
    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .await
        .unwrap();
}
