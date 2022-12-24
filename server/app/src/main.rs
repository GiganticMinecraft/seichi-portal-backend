use axum::handler::Handler;
use axum::http::header::CONTENT_TYPE;
use axum::http::Method;
use axum::routing::post;
use axum::{Router, ServiceExt};
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

// #[cfg(test)]
// mod tests {
//     use crate::forms::handlers::domain_for_user_input::raw_form::RawForm;
//     use actix_web::{dev::Service, http, test, App};
//
//     use super::*;
//
//     #[actix_web::test]
//     async fn test_index() {
//         let app = test::init_service(App::new().service(create_form_handler)).await;
//
//         let req = test::TestRequest::post()
//             .uri("/api/forms/create")
//             .set_json(&RawForm {
//                 form_name: "test1".to_owned(),
//                 form_id: 1,
//             })
//             .to_request();
//         let resp = app.call(req).await.unwrap();
//
//         assert_eq!(resp.status(), http::StatusCode::OK);
//     }
// }
