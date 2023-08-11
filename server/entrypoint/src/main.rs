use std::net::SocketAddr;

use axum::{
    http::{header::CONTENT_TYPE, Method},
    middleware,
    routing::{get, post},
    Router,
};
use common::config::{ENV, HTTP};
use hyper::header::AUTHORIZATION;
use presentation::{
    auth::auth,
    form_handler::{
        create_form_handler, delete_form_handler, form_list_handler, get_form_handler,
        post_answer_handler, update_form_handler,
    },
    health_check_handler::health_check,
};
use resource::{database::connection::ConnectionPool, repository::Repository};
use sentry::integrations::tower::{NewSentryLayer, SentryHttpLayer};
use tokio::signal::unix::{signal, SignalKind};
use tower_http::cors::{Any, CorsLayer};
use tracing::log;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Layer};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(sentry::integrations::tracing::layer())
        .with(
            tracing_subscriber::fmt::layer().with_filter(tracing_subscriber::EnvFilter::new(
                std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
            )),
        )
        .init();

    let _guard = if ENV.name != "local" {
        let _guard = sentry::init((
            "https://d1ea6a96248343c8a5dc9375d25363f0@sentry.onp.admin.seichi.click/7",
            sentry::ClientOptions {
                release: sentry::release_name!(),
                traces_sample_rate: 1.0,
                enable_profiling: true,
                profiles_sample_rate: 1.0,
                environment: Some(ENV.name.to_owned().into()),
                ..Default::default()
            },
        ));
        sentry::configure_scope(|scope| scope.set_level(Some(sentry::Level::Warning)));
        Some(_guard)
    } else {
        None
    };

    let layer = tower::ServiceBuilder::new()
        .layer(NewSentryLayer::new_from_top())
        .layer(SentryHttpLayer::with_transaction());

    let conn = ConnectionPool::new().await;
    conn.migrate().await?;

    let shared_repository = Repository::new(conn).into_shared();

    let router = Router::new()
        .route("/forms", post(create_form_handler).get(form_list_handler))
        .with_state(shared_repository.to_owned())
        .route(
            "/forms/:id",
            get(get_form_handler)
                .delete(delete_form_handler)
                .patch(update_form_handler),
        )
        .with_state(shared_repository.to_owned())
        .route("/forms/answers", post(post_answer_handler))
        .with_state(shared_repository.to_owned())
        .route("/health", get(health_check))
        .layer(layer)
        .route_layer(middleware::from_fn(auth))
        .layer(
            CorsLayer::new()
                .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::PATCH])
                .allow_origin(Any) // todo: allow_originを制限する
                .allow_headers([CONTENT_TYPE, AUTHORIZATION]),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], HTTP.port.parse().unwrap()));

    log::info!("listening on {}", HTTP.port);

    axum::Server::bind(&addr)
        .serve(router.into_make_service())
        .with_graceful_shutdown(graceful_handler())
        .await
        .expect("Fail to serve.");

    Ok(())
}

async fn graceful_handler() {
    let mut sigterm = signal(SignalKind::terminate()).unwrap();

    tokio::select! {
        _ = sigterm.recv() => {
            //todo: シャットダウン時にしなければいけない処理を記述する
        }
    }
}
