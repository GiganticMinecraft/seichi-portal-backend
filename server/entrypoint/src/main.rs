use std::{future::IntoFuture, net::SocketAddr, sync::Arc};

use axum::{
    http::{
        header::{AUTHORIZATION, CONTENT_TYPE, LOCATION},
        Method, StatusCode,
    },
    middleware,
    response::IntoResponse,
    routing::{delete, get, patch, post, put},
    Json, Router,
};
use common::config::{ENV, HTTP};
use futures::future;
use hyper::header::SET_COOKIE;
use presentation::{
    auth::auth,
    handlers::{
        form::{
            answer_handler::{
                get_all_answers, get_answer_by_form_id_handler, get_answer_handler,
                post_answer_handler, update_answer_handler,
            },
            answer_label_handler::{
                create_label_for_answers, delete_label_for_answers, edit_label_for_answers,
                get_labels_for_answers, replace_answer_labels,
            },
            comment_handler::{delete_form_comment_handler, post_form_comment},
            form_handler::{
                create_form_handler, delete_form_handler, form_list_handler, get_form_handler,
                update_form_handler,
            },
            form_label_handler::{
                create_label_for_forms, delete_label_for_forms, edit_label_for_forms,
                get_labels_for_forms, replace_form_labels,
            },
            message_handler::{
                delete_message_handler, get_messages_handler, post_message_handler,
                update_message_handler,
            },
            question_handler::{
                create_question_handler, get_questions_handler, put_question_handler,
            },
        },
        health_check_handler::health_check,
        notification_handler::{fetch_by_request_user, update_read_state},
        search_handler::cross_search,
        user_handler::{
            end_session, get_my_user_info, link_discord, patch_user_role, start_session, user_list,
        },
    },
};
use resource::{database::connection::ConnectionPool, repository::Repository};
use sentry::integrations::tower::{NewSentryLayer, SentryHttpLayer};
use serde_json::json;
use serenity::all::ShardManager;
use tokio::{net::TcpListener, signal};
use tower_http::cors::{Any, CorsLayer};
use tracing::{info, log};
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

    let mut discord_connection = resource::outgoing::connection::ConnectionPool::new().await;

    let shared_repository = Repository::new(conn).into_shared();

    let router = Router::new()
        .route("/forms", post(create_form_handler).get(form_list_handler))
        .with_state(shared_repository.to_owned())
        .route(
            "/forms/{id}",
            get(get_form_handler)
                .delete(delete_form_handler)
                .patch(update_form_handler),
        )
        .with_state(shared_repository.to_owned())
        .route("/forms/{id}/answers", get(get_answer_by_form_id_handler))
        .with_state(shared_repository.to_owned())
        .route("/forms/{id}/questions", get(get_questions_handler))
        .with_state(shared_repository.to_owned())
        .route(
            "/forms/answers",
            post(post_answer_handler).get(get_all_answers),
        )
        .with_state(shared_repository.to_owned())
        .route(
            "/forms/labels/answers",
            get(get_labels_for_answers).post(create_label_for_answers),
        )
        .with_state(shared_repository.to_owned())
        .route(
            "/forms/labels/forms",
            get(get_labels_for_forms).post(create_label_for_forms),
        )
        .with_state(shared_repository.to_owned())
        .route(
            "/forms/labels/answers/{label_id}",
            delete(delete_label_for_answers).patch(edit_label_for_answers),
        )
        .with_state(shared_repository.to_owned())
        .route(
            "/forms/labels/forms/{label_id}",
            delete(delete_label_for_forms).patch(edit_label_for_forms),
        )
        .with_state(shared_repository.to_owned())
        .route(
            "/forms/answers/{answer_id}",
            get(get_answer_handler).patch(update_answer_handler),
        )
        .with_state(shared_repository.to_owned())
        .route(
            "/forms/answers/{answer_id}/labels",
            put(replace_answer_labels),
        )
        .with_state(shared_repository.to_owned())
        .route("/forms/{form_id}/labels", put(replace_form_labels))
        .route("/forms/answers/comment", post(post_form_comment))
        .with_state(shared_repository.to_owned())
        .route(
            "/forms/answers/comments/{comment_id}",
            delete(delete_form_comment_handler),
        )
        .route(
            "/forms/questions",
            post(create_question_handler).put(put_question_handler),
        )
        .with_state(shared_repository.to_owned())
        .route("/users", get(get_my_user_info))
        .route("/users/list", get(user_list))
        .with_state(shared_repository.to_owned())
        .route("/users/{uuid}", patch(patch_user_role))
        .with_state(shared_repository.to_owned())
        .route("/search", get(cross_search))
        .with_state(shared_repository.to_owned())
        .route(
            "/forms/answers/{answer_id}/messages",
            get(get_messages_handler).post(post_message_handler),
        )
        .with_state(shared_repository.to_owned())
        .route(
            "/forms/answers/{answer_id}/messages/{message_id}",
            delete(delete_message_handler).patch(update_message_handler),
        )
        .with_state(shared_repository.to_owned())
        .route(
            "/notifications",
            get(fetch_by_request_user).patch(update_read_state),
        )
        .with_state(shared_repository.to_owned())
        .route("/health", get(health_check))
        .route("/session", post(start_session).delete(end_session))
        .with_state(shared_repository.to_owned())
        .route("/link-discord", post(link_discord))
        .with_state(shared_repository.to_owned())
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

    let shared_manager = discord_connection.pool.shard_manager.clone();

    let (_discord, _axum) = future::join(
        discord_connection.pool.start(),
        axum::serve(listener, router)
            .with_graceful_shutdown(graceful_handler(shared_manager))
            .into_future(),
    )
    .await;

    Ok(())
}

async fn not_found_handler() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(json!({ "reason": "ACCESS TO UNKNOWN ENDPOINT." })),
    )
        .into_response()
}

async fn graceful_handler(serenity_shared_manager: Arc<ShardManager>) {
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
        },
        _ = terminate => {},
    }
}
