use std::{future::IntoFuture, net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    Extension, Json, Router,
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
use domain::search::models::SearchSyncEvent;
use entrypoint::{
    logging, openapi, panic_hook, profiling, session::SessionConfig, telemetry,
    turnstile::TurnstileConfig,
};
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
use presentation::session::SessionPolicy;
use presentation::turnstile::{TurnstileState, middleware as turnstile_middleware};
use resource::object_storage::GarageObjectStorage;
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
use tracing::{info, log, warn};
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};
use utoipa_swagger_ui::SwaggerUi;

/// graceful shutdown で処理中のリクエストの完了を待つ上限。
///
/// 外向き HTTP などが詰まったリクエストが残っても、この時間で打ち切って
/// OTel / Pyroscope の flush に進み、terminationGracePeriodSeconds 内にプロセスを終える。
const HTTP_DRAIN_TIMEOUT: Duration = Duration::from_secs(15);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let turnstile_config = TurnstileConfig::from_environment()?;
    let session_config = SessionConfig::from_environment()?;
    let telemetry_providers = telemetry::init_providers();

    let stdout_log_layer = logging::stdout_log_layer(
        logging::json_logs_enabled(
            ENV.name.as_str(),
            std::env::var("LOG_FORMAT").ok().as_deref(),
        ),
        std::env::var("RUST_LOG").ok().as_deref(),
    );

    tracing_subscriber::registry()
        .with(telemetry_providers.as_ref().map(|providers| {
            tracing_opentelemetry::layer()
                .with_tracer(providers.tracer_provider.tracer("seichi-portal-backend"))
                .with_filter(telemetry::otel_span_filter())
        }))
        .with(stdout_log_layer)
        .init();
    panic_hook::install();

    // 継続プロファイリング (Grafana Pyroscope への push)。
    // PYROSCOPE_SERVER_ADDRESS 未設定なら無効。プロセスの生存期間中
    // agent を動かし続け、graceful shutdown 後に flush する
    let pyroscope_agent = profiling::start_agent();

    let conn = ConnectionPool::new().await;
    conn.migrate().await?;

    // コールバック型のメトリクスはハンドルを drop すると外れうるため、プロセス終了まで保持する
    let _observable_metrics = (
        conn.register_pool_metrics(),
        telemetry::register_runtime_metrics(tokio::runtime::Handle::current()),
    );

    let mut discord_connection = resource::outgoing::connection::ConnectionPool::new().await;

    let (sender, receiver) = mpsc::channel::<SearchSyncEvent>(100);

    let messaging_conn = resource::messaging::connection::MessagingConnectionPool::new(sender);

    let shared_manager = discord_connection.pool.shard_manager.clone();
    let messaging_conn = Arc::new(messaging_conn);

    let health_check_repo = Arc::new(resource::health_check::HealthCheckRepositoryImpl::new(
        Arc::new(conn.clone()),
        messaging_conn.clone(),
        shared_manager.clone(),
    ));
    let repository = Repository::new(conn);
    let repository = if std::env::var_os("S3_ENDPOINT").is_some() {
        repository.with_object_storage(Arc::new(GarageObjectStorage::from_environment()?))
    } else {
        tracing::warn!("S3_ENDPOINT is not set; comment attachment uploads are disabled");
        repository
    };
    let shared_repository = repository.into_shared(health_check_repo);

    let rate_limit_store =
        Arc::new(ValkeyRateLimitStore::from_environment().map_err(|error| {
            anyhow::anyhow!("invalid Valkey rate-limit configuration: {error:?}")
        })?);
    let proxy_secret = std::env::var("SEICHI_PROXY_SECRET").ok();
    let rate_limit_state = RateLimitState::new(rate_limit_store, proxy_secret.clone());
    let session_policy =
        SessionPolicy::new(session_config.maximum_lifetime_seconds(), proxy_secret);
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
        // 流れの種類の属性と trace ID 付きアクセスログ (OtelAxumLayer より内側に置く)
        .layer(middleware::from_fn(telemetry::record_request))
        // レスポンスヘッダーへの trace context 挿入 (OtelAxumLayer より内側に置く)
        .layer(OtelInResponseLayer)
        // リクエストごとの OTel スパン開始。/health はトレース対象外
        .layer(OtelAxumLayer::default().filter(|path| !path.starts_with("/health")))
        .layer(Extension(session_policy))
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
    let (drain_started_sender, drain_started_receiver) = watch::channel(false);
    let (search_engine_initialization_sender, search_engine_initialization_receiver) =
        watch::channel(SearchEngineInitializationStatus::Pending);
    let messaging_search_engine_initialization = search_engine_initialization_receiver.clone();

    let (_discord, _axum, _search_engine_initializer, _syncer, _messaging, _auto_of_sync_watcher) = join!(
        discord_connection.pool.start(),
        serve_with_drain_timeout(
            axum::serve(
                listener,
                app.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(graceful_handler(
                shared_manager,
                messaging_conn.clone(),
                search_sync_shutdown_sender.clone(),
                search_engine_initialization_sender.clone(),
                drain_started_sender,
            ))
            .into_future(),
            drain_started_receiver,
        ),
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

    if let Some(providers) = telemetry_providers {
        // shutdown() は残りのスパンとメトリクスを blocking export するため専用スレッドで行う
        tokio::task::spawn_blocking(move || {
            providers.shutdown().into_iter().for_each(|error| {
                info!("failed to shutdown OpenTelemetry {error}");
            });
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

/// axum の graceful shutdown に [`HTTP_DRAIN_TIMEOUT`] の上限を付ける。
///
/// `drain_started` が true になって (= graceful_handler が完了して axum が新規接続の受付を止め、
/// 処理中のリクエストの完了待ちに入って) から上限を数える。上限を超えたら serve の future を drop して
/// 戻る。残った接続のタスクはプロセス終了とともに破棄される。
async fn serve_with_drain_timeout(
    serve: impl Future<Output = std::io::Result<()>>,
    mut drain_started: watch::Receiver<bool>,
) -> std::io::Result<()> {
    let drain_deadline = async {
        if drain_started.wait_for(|started| *started).await.is_err() {
            // graceful_handler が通知せずに終わることは無いが、念のため上限を掛けずに serve の完了を待つ
            std::future::pending::<()>().await;
        }
        tokio::time::sleep(HTTP_DRAIN_TIMEOUT).await;
    };

    tokio::select! {
        result = serve => result,
        () = drain_deadline => {
            warn!(
                timeout = ?HTTP_DRAIN_TIMEOUT,
                "in-flight HTTP requests did not finish before the drain timeout; abandoning them"
            );
            Ok(())
        }
    }
}

async fn graceful_handler(
    serenity_shared_manager: Arc<ShardManager>,
    messaging_connection: Arc<resource::messaging::connection::MessagingConnectionPool>,
    search_sync_shutdown_sender: watch::Sender<bool>,
    search_engine_initialization_sender: watch::Sender<SearchEngineInitializationStatus>,
    drain_started_sender: watch::Sender<bool>,
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
    // この関数が返ると axum が処理中のリクエストの drain に入るので、そこから上限を数え始める
    let _ = drain_started_sender.send(true);
}
