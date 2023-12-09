use std::net::SocketAddr;

use axum::{
    http::{
        header::{AUTHORIZATION, CONTENT_TYPE, LOCATION},
        Method,
    },
    middleware,
    routing::{get, patch, post},
    Router,
};
use common::config::{ENV, HTTP};
use presentation::{
    auth::auth,
    form_handler::{
        create_form_handler, create_question_handler, delete_form_handler, form_list_handler,
        get_all_answers, get_form_handler, get_questions_handler, post_answer_handler,
        post_form_comment, update_form_handler,
    },
    health_check_handler::health_check,
    user_handler::{get_my_user_info, patch_user_role},
};
use resource::{database::connection::ConnectionPool, repository::Repository};
use sentry::integrations::tower::{NewSentryLayer, SentryHttpLayer};
use tokio::net::TcpListener;
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
        .route("/forms/:id/questions", get(get_questions_handler))
        .with_state(shared_repository.to_owned())
        .route(
            "/forms/answers",
            post(post_answer_handler).get(get_all_answers),
        )
        .with_state(shared_repository.to_owned())
        .route("/forms/answers/comment", post(post_form_comment))
        .with_state(shared_repository.to_owned())
        .route("/forms/questions", post(create_question_handler))
        .with_state(shared_repository.to_owned())
        .route("/users", get(get_my_user_info))
        .route("/users/:uuid", patch(patch_user_role))
        .with_state(shared_repository.to_owned())
        .route("/health", get(health_check))
        .layer(layer)
        .route_layer(middleware::from_fn_with_state(
            shared_repository.to_owned(),
            auth,
        ))
        .layer(
            CorsLayer::new()
                .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::PATCH])
                .allow_origin(Any) // todo: allow_originを制限する
                .allow_headers([CONTENT_TYPE, AUTHORIZATION])
                .expose_headers([LOCATION]),
        );

    let addr = SocketAddr::from(([0, 0, 0, 0], HTTP.port.parse().unwrap()));

    log::info!("listening on {}", HTTP.port);

    let listener = TcpListener::bind(addr).await.unwrap();

    axum::serve(listener, router).await.expect("Fail to serve.");
    Ok(())
}

// NOTE: hyper::Serverが削除され、2023/12/03時点でgraceful_shutdownが実装できない
// ref: https://github.com/hyperium/hyper/issues/2862
// async fn graceful_handler() {
//     let mut sigterm = signal(SignalKind::terminate()).unwrap();
//
//     tokio::select! {
//         _ = sigterm.recv() => {
//             //todo: シャットダウン時にしなければいけない処理を記述する
//         }
//     }
// }
