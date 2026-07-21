use std::sync::Arc;

use axum::{
    body::Body,
    extract::State,
    http::{Method, Request, StatusCode},
    middleware::Next,
    response::Response,
};
use domain::account::models::AccountUser;
use resource::outgoing::{
    DiscordGlobalWebhookUrl,
    discord_webhook_sender::{DiscordWebhookField, DiscordWebhookMessage, DiscordWebhookSender},
};
use tracing::warn;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormOperationNotification {
    method: Method,
    path: String,
    status: StatusCode,
    environment: String,
    requester: Requester,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Requester {
    Authenticated {
        user_id: String,
        display_name: String,
    },
    Anonymous,
}

trait FormOperationNotifier: Send + Sync {
    fn notify(&self, notification: FormOperationNotification);
}

#[derive(Clone)]
pub struct FormOperationNotificationState {
    notifier: Arc<dyn FormOperationNotifier>,
    environment: String,
}

impl FormOperationNotificationState {
    pub fn new(webhook_url: Option<DiscordGlobalWebhookUrl>, environment: String) -> Self {
        Self {
            notifier: Arc::new(DiscordFormOperationNotifier {
                webhook_url,
                sender: DiscordWebhookSender::new(),
            }),
            environment,
        }
    }

    #[cfg(test)]
    fn with_notifier(
        notifier: Arc<dyn FormOperationNotifier>,
        environment: impl Into<String>,
    ) -> Self {
        Self {
            notifier,
            environment: environment.into(),
        }
    }
}

pub async fn notify_successful_form_operation(
    State(state): State<FormOperationNotificationState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let requester = request
        .extensions()
        .get::<AccountUser>()
        .map(|user| Requester::Authenticated {
            user_id: user.id().to_string(),
            display_name: user.name().to_owned(),
        })
        .unwrap_or(Requester::Anonymous);

    let response = next.run(request).await;

    if response.status().is_success() {
        state.notifier.notify(FormOperationNotification {
            method,
            path,
            status: response.status(),
            environment: state.environment,
            requester,
        });
    }

    response
}

struct DiscordFormOperationNotifier {
    webhook_url: Option<DiscordGlobalWebhookUrl>,
    sender: DiscordWebhookSender,
}

impl FormOperationNotifier for DiscordFormOperationNotifier {
    fn notify(&self, notification: FormOperationNotification) {
        let Some(discord_webhook_url) = self.webhook_url.clone() else {
            return;
        };
        let sender = self.sender.clone();

        tokio::spawn(async move {
            let attempts = DiscordWebhookSender::retry_policy().max_attempts();
            let message = DiscordWebhookMessage {
                discord_webhook_url: discord_webhook_url.into_inner(),
                title: "フォーム操作が完了しました".to_string(),
                link_url: None,
                fields: notification.into_fields(),
            };

            if let Err(error) = sender.send_with_retry(message).await {
                warn!(
                    attempts,
                    error = %error,
                    "failed to send global Discord webhook after retries"
                );
            }
        });
    }
}

impl FormOperationNotification {
    fn into_fields(self) -> Vec<DiscordWebhookField> {
        let operation_fields = [
            DiscordWebhookField::new("Method".to_string(), self.method.to_string(), true),
            DiscordWebhookField::new("Path".to_string(), self.path, false),
            DiscordWebhookField::new("Status".to_string(), self.status.as_u16().to_string(), true),
            DiscordWebhookField::new("Environment".to_string(), self.environment, true),
        ];

        let requester_fields = match self.requester {
            Requester::Authenticated {
                user_id,
                display_name,
            } => vec![
                DiscordWebhookField::new("User ID".to_string(), user_id, true),
                DiscordWebhookField::new("Display Name".to_string(), display_name, true),
            ],
            Requester::Anonymous => vec![DiscordWebhookField::new(
                "User".to_string(),
                "Anonymous".to_string(),
                true,
            )],
        };

        operation_fields
            .into_iter()
            .chain(requester_fields)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        middleware,
        response::IntoResponse,
        routing::post,
    };
    use domain::account::models::{AccountUser, Role, UserId};
    use tower::ServiceExt;
    use uuid::Uuid;

    use super::{
        FormOperationNotification, FormOperationNotificationState, FormOperationNotifier,
        Requester, notify_successful_form_operation,
    };

    #[derive(Default)]
    struct RecordingNotifier(Mutex<Vec<FormOperationNotification>>);

    impl FormOperationNotifier for RecordingNotifier {
        fn notify(&self, notification: FormOperationNotification) {
            self.0.lock().unwrap().push(notification);
        }
    }

    fn app(status: StatusCode, notifier: Arc<RecordingNotifier>) -> Router {
        let state = FormOperationNotificationState::with_notifier(notifier, "test");

        Router::new()
            .route(
                "/forms/{id}",
                post(move || async move { status.into_response() }),
            )
            .route_layer(middleware::from_fn_with_state(
                state,
                notify_successful_form_operation,
            ))
    }

    #[tokio::test]
    async fn sends_notification_only_for_successful_responses() {
        let success_notifier = Arc::new(RecordingNotifier::default());
        let failure_notifier = Arc::new(RecordingNotifier::default());

        let success_response = app(StatusCode::NO_CONTENT, success_notifier.clone())
            .oneshot(
                Request::post("/forms/actual-id?secret=query")
                    .header("authorization", "Bearer secret")
                    .body(Body::from("secret body"))
                    .unwrap(),
            )
            .await
            .unwrap();
        let failure_response = app(StatusCode::BAD_REQUEST, failure_notifier.clone())
            .oneshot(
                Request::post("/forms/actual-id")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(success_response.status(), StatusCode::NO_CONTENT);
        assert_eq!(failure_response.status(), StatusCode::BAD_REQUEST);
        let notifications = success_notifier.0.lock().unwrap();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].path, "/forms/actual-id");
        assert_eq!(notifications[0].requester, Requester::Anonymous);
        assert!(failure_notifier.0.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn includes_authenticated_user_identity() {
        let notifier = Arc::new(RecordingNotifier::default());
        let user_id = UserId::from(Uuid::new_v4());
        let mut request = Request::post("/forms/actual-id")
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(AccountUser::new(
            "player-name".to_string(),
            user_id,
            Role::StandardUser,
        ));

        app(StatusCode::OK, notifier.clone())
            .oneshot(request)
            .await
            .unwrap();

        let notifications = notifier.0.lock().unwrap();
        assert_eq!(notifications.len(), 1);
        assert_eq!(
            notifications[0].requester,
            Requester::Authenticated {
                user_id: user_id.to_string(),
                display_name: "player-name".to_string(),
            }
        );
    }
}
