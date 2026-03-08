use std::{future::IntoFuture, net::SocketAddr, sync::Arc};

use axum::{
    Json, Router,
    http::{
        Method, StatusCode,
        header::{AUTHORIZATION, CONTENT_TYPE, LOCATION},
    },
    middleware,
    response::IntoResponse,
    routing::post,
};
use common::config::{ENV, HTTP};
use domain::search::models::SearchableFieldsWithOperation;
use futures::join;
use hyper::header::SET_COOKIE;
use presentation::api::notification_api_impl::NotificationAPIImpl;
use presentation::auth::auth;
use presentation::handlers::form::message_handler::{
    RealInfrastructureRepositoryWithNotificationAPI, post_message_handler,
};
use presentation::handlers::search_handler::{
    initialize_search_engine, start_sync, start_watch_out_of_sync,
};
use resource::{database::connection::ConnectionPool, repository::Repository};
use sentry::integrations::tower::{NewSentryLayer, SentryHttpLayer};
use serde_json::json;
use serenity::all::ShardManager;
use tokio::{
    net::TcpListener,
    signal,
    sync::{Notify, mpsc},
};
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, log};
use tracing_subscriber::{Layer, layer::SubscriberExt, util::SubscriberInitExt};
use usecase::notification::discord_dm_notificator_impl::DiscordDMNotificatorImpl;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;
use utoipa_swagger_ui::SwaggerUi;

#[derive(OpenApi)]
#[openapi(
    paths(
        presentation::handlers::form::message_handler::post_message_handler,
    ),
    info(title = "Seichi Portal API", version = "1.0.0"),
    components(schemas(
        presentation::schemas::error_response::ErrorResponse,
        presentation::schemas::user::UserInfoResponse,
        presentation::schemas::user::UserSchema,
        presentation::schemas::form::form_response_schemas::AnswerComment,
        presentation::schemas::form::form_response_schemas::AnswerContent,
        presentation::schemas::form::form_response_schemas::AnswerLabels,
        presentation::schemas::form::form_response_schemas::AnswerLabelResponseSchema,
        presentation::schemas::form::form_response_schemas::AnswerSettingsSchema,
        presentation::schemas::form::form_response_schemas::AnswerVisibility,
        presentation::schemas::form::form_response_schemas::FormAnswer,
        presentation::schemas::form::form_response_schemas::FormLabelResponseSchema,
        presentation::schemas::form::form_response_schemas::FormMetaSchema,
        presentation::schemas::form::form_response_schemas::FormSchema,
        presentation::schemas::form::form_response_schemas::FormSettingsSchema,
        presentation::schemas::form::form_response_schemas::MessageContentSchema,
        presentation::schemas::form::form_response_schemas::PutQuestionsResponseSchema,
        presentation::schemas::form::form_response_schemas::QuestionResponseSchema,
        presentation::schemas::form::form_response_schemas::ResponsePeriodSchema,
        presentation::schemas::form::form_response_schemas::Role,
        presentation::schemas::form::form_response_schemas::SenderSchema,
        presentation::schemas::form::form_response_schemas::User,
        presentation::schemas::notification::notification_response_schemas::NotificationSettingsResponse,
        presentation::schemas::search_schemas::CommentSchema,
        presentation::schemas::search_schemas::CrossSearchResult,
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "Forms"),
        (name = "Answers"),
        (name = "Questions"),
        (name = "Comments"),
        (name = "Labels"),
        (name = "Messages"),
        (name = "Users"),
        (name = "Search"),
        (name = "Notifications"),
        (name = "Session"),
        (name = "Health"),
    )
)]
struct ApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer",
                SecurityScheme::Http(HttpBuilder::new().scheme(HttpAuthScheme::Bearer).build()),
            );
        }
    }
}

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
            "http://481e89e767984164a62dfcb92c869db6@bugsink.seichi-minecraft/1",
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
        .layer(SentryHttpLayer::new().enable_transaction());

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
    let notificator_impl = DiscordDMNotificatorImpl::new();
    let notification_api = NotificationAPIImpl::new(discord_sender, notificator_impl);

    use presentation::handlers::form::{
        answer_handler, answer_label_handler, comment_handler, form_handler, form_label_handler,
        message_handler, question_handler,
    };
    use presentation::handlers::{
        health_check_handler, notification_handler, search_handler, user_handler,
    };

    let (router, openapi) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .routes(routes!(
            form_handler::create_form_handler,
            form_handler::form_list_handler
        ))
        .routes(routes!(
            form_handler::get_form_handler,
            form_handler::delete_form_handler,
            form_handler::update_form_handler
        ))
        .routes(routes!(
            answer_handler::get_answer_by_form_id_handler,
            answer_handler::post_answer_handler
        ))
        .routes(routes!(
            question_handler::get_questions_handler,
            question_handler::put_question_handler
        ))
        .routes(routes!(answer_handler::get_all_answers))
        .routes(routes!(
            answer_label_handler::get_labels_for_answers,
            answer_label_handler::create_label_for_answers
        ))
        .routes(routes!(
            answer_label_handler::delete_label_for_answers,
            answer_label_handler::edit_label_for_answers
        ))
        .routes(routes!(
            form_label_handler::get_labels_for_forms,
            form_label_handler::create_label_for_forms
        ))
        .routes(routes!(
            form_label_handler::delete_label_for_forms,
            form_label_handler::edit_label_for_forms
        ))
        .routes(routes!(
            answer_handler::get_answer_handler,
            answer_handler::update_answer_handler
        ))
        .routes(routes!(answer_label_handler::replace_answer_labels))
        .routes(routes!(form_label_handler::replace_form_labels))
        .routes(routes!(
            comment_handler::get_form_comment,
            comment_handler::post_form_comment
        ))
        .routes(routes!(
            comment_handler::update_form_comment,
            comment_handler::delete_form_comment_handler
        ))
        .routes(routes!(
            user_handler::get_user_info,
            user_handler::patch_user_role
        ))
        .routes(routes!(user_handler::get_my_user_info))
        .routes(routes!(user_handler::user_list))
        .routes(routes!(search_handler::cross_search))
        .routes(routes!(message_handler::get_messages_handler))
        .routes(routes!(
            message_handler::update_message_handler,
            message_handler::delete_message_handler
        ))
        .routes(routes!(notification_handler::get_notification_settings))
        .routes(routes!(
            notification_handler::get_my_notification_settings,
            notification_handler::update_notification_settings
        ))
        .routes(routes!(health_check_handler::health_check))
        .routes(routes!(
            user_handler::start_session,
            user_handler::end_session
        ))
        .routes(routes!(
            user_handler::link_discord,
            user_handler::unlink_discord
        ))
        .with_state(shared_repository.to_owned())
        .split_for_parts();

    // post_message_handler uses a different State type, so register it separately
    let message_post_router = Router::new()
        .route(
            "/forms/{form_id}/answers/{answer_id}/messages",
            post(post_message_handler),
        )
        .with_state(Arc::new(
            RealInfrastructureRepositoryWithNotificationAPI::new(
                shared_repository.to_owned(),
                notification_api,
            ),
        ));

    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", openapi))
        .merge(router)
        .merge(message_post_router)
        .fallback(not_found_handler)
        .layer(layer)
        .route_layer(middleware::from_fn_with_state(
            shared_repository.to_owned(),
            auth,
        ))
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
        );

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
