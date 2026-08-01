use std::{future::IntoFuture, net::SocketAddr, sync::Arc};

use axum::{
    Json, Router,
    http::{
        Method, StatusCode,
        header::{AUTHORIZATION, CONTENT_TYPE, LOCATION},
    },
    middleware,
    response::IntoResponse,
    routing::{get, post},
};
use axum_tracing_opentelemetry::middleware::{OtelAxumLayer, OtelInResponseLayer};
use common::config::{ENV, HTTP};
use domain::search::models::SearchableFieldsWithOperation;
use entrypoint::{logging, openapi, panic_hook, telemetry};
use futures::join;
use hyper::header::SET_COOKIE;
use opentelemetry::trace::TracerProvider as _;
use presentation::api::global_discord_webhook::start_global_discord_webhook_worker;
use presentation::api::notificator_impl::DiscordNotificator;
use presentation::auth::{auth, optional_auth};
use presentation::handlers::form::message_handler::{
    RealInfrastructureRepositoryWithNotificator, post_message_handler,
};
use presentation::handlers::search_handler::{
    initialize_search_engine, start_sync, start_watch_out_of_sync,
};
use resource::{database::connection::ConnectionPool, repository::Repository};
use serde_json::json;
use serenity::all::ShardManager;
use tokio::{
    net::TcpListener,
    signal,
    sync::{Notify, mpsc},
};
use tower_http::{
    catch_panic::CatchPanicLayer,
    cors::{Any, CorsLayer},
};
use tracing::{info, log};
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};
use utoipa_swagger_ui::SwaggerUi;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let tracer_provider = telemetry::init_tracer_provider();

    // SQL 文の出力 (bind 値を含みうる) はログへ出さない
    let stdout_log_filter = || {
        tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        )
        .add_directive("sqlx::query=off".parse().expect("directive must be valid"))
    };
    let json_logs_enabled = logging::json_logs_enabled(
        ENV.name.as_str(),
        std::env::var("LOG_FORMAT").ok().as_deref(),
    );
    let (json_log_layer, pretty_log_layer) = if json_logs_enabled {
        (
            Some(logging::json_log_layer().with_filter(stdout_log_filter())),
            None,
        )
    } else {
        (
            None,
            Some(tracing_subscriber::fmt::layer().with_filter(stdout_log_filter())),
        )
    };

    tracing_subscriber::registry()
        .with(tracer_provider.as_ref().map(|provider| {
            tracing_opentelemetry::layer().with_tracer(provider.tracer("seichi-portal-backend"))
        }))
        .with(json_log_layer)
        .with(pretty_log_layer)
        .init();
    panic_hook::install();

    let conn = ConnectionPool::new().await;
    conn.migrate().await?;

    let mut discord_connection = resource::outgoing::connection::ConnectionPool::new().await;

    let (sender, receiver) = mpsc::channel::<SearchableFieldsWithOperation>(100);

    let messaging_conn =
        resource::messaging::connection::MessagingConnectionPool::new(sender).await;

    let shared_manager = discord_connection.pool.shard_manager.clone();
    let messaging_conn = Arc::new(messaging_conn);

    let health_check_repo = Arc::new(resource::health_check::HealthCheckRepositoryImpl::new(
        Arc::new(conn.clone()),
        messaging_conn.clone(),
        shared_manager.clone(),
    ));
    let shared_repository = Repository::new(conn).into_shared(health_check_repo);

    let discord_sender = resource::outgoing::connection::ConnectionPool::new().await;
    let notificator = DiscordNotificator::new(discord_sender, shared_repository.to_owned());
    let _global_discord_webhook_worker =
        start_global_discord_webhook_worker(shared_repository.to_owned());

    use presentation::handlers::health_check_handler;

    let openapi = openapi::versioned_api_router().into_openapi();

    let (public_api, _) = openapi::public_api_router()
        .with_state(shared_repository.to_owned())
        .split_for_parts();

    let (optional_auth_api, _) = openapi::optional_auth_api_router()
        .with_state(shared_repository.to_owned())
        .split_for_parts();
    let optional_auth_api = optional_auth_api.route_layer(middleware::from_fn_with_state(
        shared_repository.to_owned(),
        optional_auth,
    ));

    let (authenticated_api, _) = openapi::authenticated_api_router()
        .with_state(shared_repository.to_owned())
        .split_for_parts();
    let authenticated_api = authenticated_api.route_layer(middleware::from_fn_with_state(
        shared_repository.to_owned(),
        auth,
    ));

    // post_message_handler uses a different State type, so register it separately
    let message_post_router = Router::new()
        .route(
            "/forms/{form_id}/answers/{answer_id}/messages",
            post(post_message_handler),
        )
        .route_layer(middleware::from_fn_with_state(
            shared_repository.to_owned(),
            auth,
        ))
        .with_state(Arc::new(RealInfrastructureRepositoryWithNotificator::new(
            shared_repository.to_owned(),
            notificator,
        )));

    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", openapi))
        .route("/health", get(health_check_handler::health_check))
        .nest(
            "/api/v1",
            public_api
                .merge(optional_auth_api)
                .merge(authenticated_api)
                .merge(message_post_router),
        )
        .fallback(not_found_handler)
        // handler 内 panic で 500 を返し、コネクションを維持する
        .layer(CatchPanicLayer::new())
        // レスポンスヘッダーへの trace context 挿入 (OtelAxumLayer より内側に置く)
        .layer(OtelInResponseLayer)
        // リクエストごとの OTel スパン開始。/health はトレース対象外
        .layer(OtelAxumLayer::default().filter(|path| !path.starts_with("/health")))
        .layer(
            CorsLayer::new()
                .allow_methods([
                    Method::GET,
                    Method::POST,
                    Method::DELETE,
                    Method::PATCH,
                    Method::PUT,
                ])
                .allow_origin(Any) // todo: allow_originを制限する
                .allow_headers([CONTENT_TYPE, AUTHORIZATION])
                .expose_headers([LOCATION, SET_COOKIE]),
        )
        .with_state(shared_repository.to_owned());

    let addr = SocketAddr::from(([0, 0, 0, 0], HTTP.port.parse().unwrap()));

    log::info!("listening on {}", HTTP.port);

    let listener = TcpListener::bind(addr).await.unwrap();

    let shutdown_notifier = Arc::new(Notify::new());

    initialize_search_engine(shared_repository.to_owned()).await?;

    let (_discord, _axum, _syncer, _messaging, _auto_of_sync_watcher) = join!(
        discord_connection.pool.start(),
        axum::serve(listener, app)
            .with_graceful_shutdown(graceful_handler(
                shared_manager,
                messaging_conn.clone(),
                shutdown_notifier.clone(),
            ))
            .into_future(),
        start_sync(
            shared_repository.to_owned(),
            receiver,
            shutdown_notifier.clone(),
        ),
        messaging_conn.consumer(),
        start_watch_out_of_sync(shared_repository.to_owned(), shutdown_notifier.clone())
    );

    if let Some(provider) = tracer_provider {
        // provider.shutdown() は残りのスパンを blocking export するため専用スレッドで行う
        tokio::task::spawn_blocking(move || {
            if let Err(error) = provider.shutdown() {
                info!("failed to shutdown OpenTelemetry tracer provider: {error}");
            }
        })
        .await?;
    }

    Ok(())
}

async fn not_found_handler() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "reason": "ACCESS TO UNKNOWN ENDPOINT." })),
    )
        .into_response()
}

async fn graceful_handler(
    serenity_shared_manager: Arc<ShardManager>,
    messaging_connection: Arc<resource::messaging::connection::MessagingConnectionPool>,
    search_engine_syncer_shutdown_notifier: Arc<Notify>,
) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Gracefully shutdown...");
            serenity_shared_manager.shutdown_all().await;
            messaging_connection.shutdown().await;
            search_engine_syncer_shutdown_notifier.notify_waiters();
        },
        _ = terminate => {},
    }
}
