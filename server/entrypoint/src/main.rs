use std::{future::IntoFuture, net::SocketAddr, sync::Arc};

use axum::{
    Json, Router,
    http::{
        HeaderName, Method, StatusCode,
        header::{AUTHORIZATION, CONTENT_TYPE, LOCATION},
    },
    middleware,
    response::IntoResponse,
    routing::{get, post},
};
use axum_tracing_opentelemetry::middleware::{OtelAxumLayer, OtelInResponseLayer};
use common::config::{ENV, HTTP};
use domain::search::models::SearchableFieldsWithOperation;
use entrypoint::{logging, openapi, panic_hook, profiling, telemetry, turnstile::TurnstileConfig};
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
    SearchEngineInitializationStatus, start_initialize_search_engine, start_sync,
    start_watch_out_of_sync, update_search_engine_initialization_status,
    wait_for_search_engine_initialization,
};
use presentation::rate_limit::{RateLimitState, middleware as rate_limit_middleware};
use presentation::turnstile::{TurnstileState, middleware as turnstile_middleware};
use resource::rate_limit::ValkeyRateLimitStore;
use resource::turnstile::TurnstileSiteverifyClient;
use resource::{database::connection::ConnectionPool, repository::Repository};
use serde_json::json;
use serenity::all::ShardManager;
use tokio::{
    net::TcpListener,
    signal,
    sync::{mpsc, watch},
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
    let turnstile_config = TurnstileConfig::from_environment()?;
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

    // 継続プロファイリング (Grafana Pyroscope への push)。
    // PYROSCOPE_SERVER_ADDRESS 未設定なら無効。プロセスの生存期間中
    // agent を動かし続け、graceful shutdown 後に flush する
    let pyroscope_agent = profiling::start_agent();

    let conn = ConnectionPool::new().await;
    conn.migrate().await?;

    let mut discord_connection = resource::outgoing::connection::ConnectionPool::new().await;

    let (sender, receiver) = mpsc::channel::<SearchableFieldsWithOperation>(100);

    let messaging_conn = resource::messaging::connection::MessagingConnectionPool::new(sender);

    let shared_manager = discord_connection.pool.shard_manager.clone();
    let messaging_conn = Arc::new(messaging_conn);

    let health_check_repo = Arc::new(resource::health_check::HealthCheckRepositoryImpl::new(
        Arc::new(conn.clone()),
        messaging_conn.clone(),
        shared_manager.clone(),
    ));
    let shared_repository = Repository::new(conn).into_shared(health_check_repo);

    let rate_limit_store =
        Arc::new(ValkeyRateLimitStore::from_environment().map_err(|error| {
            anyhow::anyhow!("invalid Valkey rate-limit configuration: {error:?}")
        })?);
    let proxy_secret = std::env::var("SEICHI_PROXY_SECRET").ok();
    let rate_limit_state = RateLimitState::new(rate_limit_store, proxy_secret);
    let turnstile_state = match turnstile_config {
        TurnstileConfig::Disabled => TurnstileState::disabled(),
        TurnstileConfig::Enabled {
            secret_key,
            allowed_hostnames,
        } => TurnstileState::enabled(
            Arc::new(TurnstileSiteverifyClient::new(secret_key)),
            allowed_hostnames,
        ),
    };

    let discord_sender = resource::outgoing::connection::ConnectionPool::new().await;
    let notificator = DiscordNotificator::new(discord_sender, shared_repository.to_owned());
    let _global_discord_webhook_worker =
        start_global_discord_webhook_worker(shared_repository.to_owned());

    use presentation::handlers::health_check_handler;

    let openapi = openapi::versioned_api_router().into_openapi();

    let (optional_auth_api, _) = openapi::optional_auth_api_router()
        .with_state(shared_repository.to_owned())
        .split_for_parts();
    let optional_auth_api = optional_auth_api
        .route_layer(middleware::from_fn_with_state(
            rate_limit_state.clone(),
            rate_limit_middleware,
        ))
        .route_layer(middleware::from_fn_with_state(
            shared_repository.to_owned(),
            optional_auth,
        ));

    let (authenticated_api, _) = openapi::authenticated_api_router()
        .with_state(shared_repository.to_owned())
        .split_for_parts();
    let authenticated_api = authenticated_api
        .route_layer(middleware::from_fn_with_state(
            rate_limit_state.clone(),
            rate_limit_middleware,
        ))
        .route_layer(middleware::from_fn_with_state(
            shared_repository.to_owned(),
            auth,
        ));

    let (authenticated_session_api, _) = openapi::authenticated_session_api_router()
        .with_state(shared_repository.to_owned())
        .split_for_parts();
    let authenticated_session_api = authenticated_session_api
        .route_layer(middleware::from_fn_with_state(
            rate_limit_state.clone(),
            rate_limit_middleware,
        ))
        .route_layer(middleware::from_fn_with_state(
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
            rate_limit_state.clone(),
            rate_limit_middleware,
        ))
        .route_layer(middleware::from_fn_with_state(
            shared_repository.to_owned(),
            auth,
        ))
        .with_state(Arc::new(RealInfrastructureRepositoryWithNotificator::new(
            shared_repository.to_owned(),
            notificator,
        )));

    let (public_api, _) = openapi::public_api_router()
        .with_state(shared_repository.to_owned())
        .split_for_parts();
    let public_api = public_api
        // route_layer は後から追加した layer が外側になるため、rate limit -> Turnstile -> handler の順にする。
        .route_layer(middleware::from_fn_with_state(
            turnstile_state,
            turnstile_middleware,
        ))
        .route_layer(middleware::from_fn_with_state(
            rate_limit_state,
            rate_limit_middleware,
        ));

    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", openapi))
        .route("/health", get(health_check_handler::health_check))
        .nest(
            "/api/v1",
            public_api
                .merge(optional_auth_api)
                .merge(authenticated_api)
                .merge(authenticated_session_api)
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
                .allow_headers([
                    CONTENT_TYPE,
                    AUTHORIZATION,
                    HeaderName::from_static("x-seichi-proxy-secret"),
                    HeaderName::from_static("x-seichi-client-ip"),
                    HeaderName::from_static("x-seichi-turnstile-token"),
                ])
                .expose_headers([
                    LOCATION,
                    SET_COOKIE,
                    HeaderName::from_static("retry-after"),
                    HeaderName::from_static("ratelimit-limit"),
                    HeaderName::from_static("ratelimit-remaining"),
                    HeaderName::from_static("ratelimit-reset"),
                ]),
        )
        .with_state(shared_repository.to_owned());

    let addr = SocketAddr::from(([0, 0, 0, 0], HTTP.port.parse().unwrap()));

    log::info!("listening on {}", HTTP.port);

    let listener = TcpListener::bind(addr).await.unwrap();

    let (search_sync_shutdown_sender, search_sync_shutdown_status) = watch::channel(false);
    let (search_engine_initialization_sender, search_engine_initialization_receiver) =
        watch::channel(SearchEngineInitializationStatus::Pending);
    let messaging_search_engine_initialization = search_engine_initialization_receiver.clone();

    let (_discord, _axum, _search_engine_initializer, _syncer, _messaging, _auto_of_sync_watcher) = join!(
        discord_connection.pool.start(),
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(graceful_handler(
            shared_manager,
            messaging_conn.clone(),
            search_sync_shutdown_sender.clone(),
            search_engine_initialization_sender.clone(),
        ))
        .into_future(),
        start_initialize_search_engine(
            shared_repository.to_owned(),
            search_engine_initialization_sender,
            search_engine_initialization_receiver.clone(),
        ),
        start_sync(
            shared_repository.to_owned(),
            receiver,
            search_sync_shutdown_status.clone(),
            search_engine_initialization_receiver.clone(),
        ),
        async move {
            if wait_for_search_engine_initialization(messaging_search_engine_initialization).await {
                messaging_conn.consumer().await
            } else {
                Ok(())
            }
        },
        start_watch_out_of_sync(
            shared_repository.to_owned(),
            search_sync_shutdown_status,
            search_engine_initialization_receiver,
        )
    );

    if let Some(agent) = pyroscope_agent {
        // stop() は最終プロファイルの送信を伴うため専用スレッドで行う
        tokio::task::spawn_blocking(move || match agent.stop() {
            Ok(agent) => agent.shutdown(),
            Err(error) => info!("failed to stop Pyroscope agent: {error}"),
        })
        .await?;
    }

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
    search_sync_shutdown_sender: watch::Sender<bool>,
    search_engine_initialization_sender: watch::Sender<SearchEngineInitializationStatus>,
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

    // SIGTERM (Kubernetes の Pod 停止) でも Ctrl+C と同じ停止経路に集約する。
    // ここで各コンポーネントを止めないと main の join! が完了せず、
    // OTel / Pyroscope の flush に到達しないまま SIGKILL される
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Gracefully shutdown...");
    let _ = search_sync_shutdown_sender.send(true);
    update_search_engine_initialization_status(
        &search_engine_initialization_sender,
        SearchEngineInitializationStatus::Shutdown,
    );
    serenity_shared_manager.shutdown_all().await;
    messaging_connection.shutdown().await;
}
