use std::{collections::HashSet, sync::Arc};

use axum::{
    body::Body,
    extract::State,
    http::{Method, Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use common::turnstile::{TurnstileVerification, TurnstileVerificationError, TurnstileVerifier};

use crate::schemas::error_response::ErrorResponse;

const API_PREFIX: &str = "/api/v1";
const MAX_TOKEN_LENGTH: usize = 2048;
pub const TURNSTILE_TOKEN_HEADER: &str = "x-seichi-turnstile-token";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TurnstileAction {
    SessionCreate,
    TemporaryAnswer,
}

impl TurnstileAction {
    fn as_str(self) -> &'static str {
        match self {
            Self::SessionCreate => "session-create",
            Self::TemporaryAnswer => "temporary-answer",
        }
    }
}

#[derive(Clone)]
pub enum TurnstileState {
    Disabled,
    Enabled {
        verifier: Arc<dyn TurnstileVerifier>,
        allowed_hostnames: Arc<HashSet<String>>,
    },
}

impl TurnstileState {
    pub fn disabled() -> Self {
        Self::Disabled
    }

    pub fn enabled(
        verifier: Arc<dyn TurnstileVerifier>,
        allowed_hostnames: HashSet<String>,
    ) -> Self {
        Self::Enabled {
            verifier,
            allowed_hostnames: Arc::new(
                allowed_hostnames
                    .into_iter()
                    .map(|hostname| hostname.to_ascii_lowercase())
                    .collect(),
            ),
        }
    }
}

fn request_path_without_api_prefix(path: &str) -> Option<&str> {
    if path == API_PREFIX {
        Some("/")
    } else if let Some(stripped) = path.strip_prefix(API_PREFIX) {
        stripped.starts_with('/').then_some(stripped)
    } else if path.starts_with('/') {
        Some(path)
    } else {
        None
    }
}

fn expected_action(method: &Method, path: &str) -> Option<TurnstileAction> {
    if *method != Method::POST {
        return None;
    }

    let path = request_path_without_api_prefix(path)?;
    let segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    match segments.as_slice() {
        ["session"] => Some(TurnstileAction::SessionCreate),
        ["forms", _, "temporary-answers"] => Some(TurnstileAction::TemporaryAnswer),
        _ => None,
    }
}

fn allowed_hostname(hostname: &str, allowed_hostnames: &HashSet<String>) -> bool {
    allowed_hostnames.contains(&hostname.to_ascii_lowercase())
}

fn problem_response(status: StatusCode, title: &str, detail: &str, error_code: &str) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "application/problem+json")],
        axum::Json(ErrorResponse {
            problem_type: "about:blank".to_owned(),
            title: title.to_owned(),
            status: status.as_u16(),
            detail: detail.to_owned(),
            error_code: error_code.to_owned(),
            restriction: None,
        }),
    )
        .into_response()
}

fn forbidden_response() -> Response {
    problem_response(
        StatusCode::FORBIDDEN,
        "Forbidden",
        "Turnstile verification failed.",
        "TURNSTILE_VERIFICATION_FAILED",
    )
}

fn unavailable_response() -> Response {
    problem_response(
        StatusCode::SERVICE_UNAVAILABLE,
        "Service Unavailable",
        "Turnstile verification service is unavailable.",
        "TURNSTILE_UNAVAILABLE",
    )
}

pub async fn middleware(
    State(state): State<TurnstileState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let Some(action) = expected_action(request.method(), request.uri().path()) else {
        return next.run(request).await;
    };

    let TurnstileState::Enabled {
        verifier,
        allowed_hostnames,
    } = state
    else {
        return next.run(request).await;
    };

    let Some(token) = request
        .headers()
        .get(TURNSTILE_TOKEN_HEADER)
        .and_then(|value| value.to_str().ok())
        .filter(|token| !token.is_empty() && token.len() <= MAX_TOKEN_LENGTH)
    else {
        return forbidden_response();
    };

    let verification = match verifier.verify(token).await {
        Ok(verification) => verification,
        Err(TurnstileVerificationError::Unavailable)
        | Err(TurnstileVerificationError::InvalidResponse) => return unavailable_response(),
    };

    let TurnstileVerification::Accepted {
        action: actual_action,
        hostname,
    } = verification
    else {
        return forbidden_response();
    };

    if actual_action.as_deref() != Some(action.as_str())
        || !hostname.is_some_and(|hostname| allowed_hostname(&hostname, &allowed_hostnames))
    {
        return forbidden_response();
    }

    next.run(request).await
}

#[cfg(test)]
mod tests {
    use std::{
        net::SocketAddr,
        sync::{Arc, Mutex},
    };

    use async_trait::async_trait;
    use axum::{
        Router,
        body::{Body, to_bytes},
        extract::{ConnectInfo, Request},
        http::{Method, StatusCode, header::HeaderValue},
        routing::post,
    };
    use common::{
        rate_limit::{RateLimitDecision, RateLimitQuota, RateLimitStore, RateLimitStoreError},
        turnstile::{TurnstileVerification, TurnstileVerificationError, TurnstileVerifier},
    };
    use tower::ServiceExt;

    use crate::rate_limit::{RateLimitState, middleware as rate_limit_middleware};

    use super::*;

    #[derive(Clone)]
    struct RecordingVerifier {
        events: Arc<Mutex<Vec<&'static str>>>,
    }

    #[async_trait]
    impl TurnstileVerifier for RecordingVerifier {
        async fn verify(
            &self,
            _token: &str,
        ) -> Result<TurnstileVerification, TurnstileVerificationError> {
            self.events.lock().unwrap().push("turnstile");
            Ok(TurnstileVerification::Accepted {
                action: Some("session-create".to_owned()),
                hostname: Some("EXAMPLE.COM".to_owned()),
            })
        }
    }

    #[derive(Clone)]
    struct FixedVerifier {
        result: Result<TurnstileVerification, TurnstileVerificationError>,
    }

    #[async_trait]
    impl TurnstileVerifier for FixedVerifier {
        async fn verify(
            &self,
            _token: &str,
        ) -> Result<TurnstileVerification, TurnstileVerificationError> {
            self.result.clone()
        }
    }

    struct RecordingRateLimitStore {
        events: Arc<Mutex<Vec<&'static str>>>,
    }

    #[async_trait]
    impl RateLimitStore for RecordingRateLimitStore {
        async fn check(
            &self,
            _quotas: &[RateLimitQuota],
        ) -> Result<RateLimitDecision, RateLimitStoreError> {
            self.events.lock().unwrap().push("rate-limit");
            Ok(RateLimitDecision {
                allowed: true,
                retry_after_seconds: 1,
                remaining: 1,
                limit: 2,
                reset_seconds: 60,
            })
        }
    }

    fn state(events: Arc<Mutex<Vec<&'static str>>>) -> TurnstileState {
        TurnstileState::enabled(
            Arc::new(RecordingVerifier { events }),
            ["example.com".to_owned()].into_iter().collect(),
        )
    }

    fn fixed_verification_app(
        result: Result<TurnstileVerification, TurnstileVerificationError>,
    ) -> Router {
        Router::new()
            .route("/session", post(|| async { StatusCode::OK }))
            .layer(axum::middleware::from_fn_with_state(
                TurnstileState::enabled(
                    Arc::new(FixedVerifier { result }),
                    ["example.com".to_owned()].into_iter().collect(),
                ),
                middleware,
            ))
    }

    #[test]
    fn only_the_two_public_post_routes_have_an_action() {
        assert_eq!(
            expected_action(&Method::POST, "/session"),
            Some(TurnstileAction::SessionCreate)
        );
        assert_eq!(
            expected_action(
                &Method::POST,
                "/api/v1/forms/018f4f37-2f5e-7b39-8b39-9a2f2695d7ad/temporary-answers"
            ),
            Some(TurnstileAction::TemporaryAnswer)
        );
        assert_eq!(expected_action(&Method::GET, "/forms"), None);
        assert_eq!(expected_action(&Method::POST, "/users"), None);
        assert_eq!(expected_action(&Method::POST, "/api/v10/session"), None);
    }

    #[test]
    fn hostname_matching_is_case_insensitive_but_not_wildcard_based() {
        let allowed = ["example.com".to_owned()].into_iter().collect();
        assert!(allowed_hostname("example.com", &allowed));
        assert!(allowed_hostname("EXAMPLE.COM", &allowed));
        assert!(!allowed_hostname("sub.example.com", &allowed));

        let wildcard = ["*.example.com".to_owned()].into_iter().collect();
        assert!(!allowed_hostname("sub.example.com", &wildcard));
    }

    #[tokio::test]
    async fn enabled_middleware_validates_without_consuming_the_body() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let app = Router::new()
            .route("/session", post(|body: String| async move { body }))
            .layer(axum::middleware::from_fn_with_state(
                state(events),
                middleware,
            ));
        let request = Request::builder()
            .method(Method::POST)
            .uri("/session")
            .header(TURNSTILE_TOKEN_HEADER, HeaderValue::from_static("token"))
            .body(Body::from("request body"))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            to_bytes(response.into_body(), usize::MAX).await.unwrap(),
            "request body"
        );
    }

    #[tokio::test]
    async fn disabled_middleware_passes_a_request_without_a_token() {
        let app = Router::new()
            .route("/session", post(|body: String| async move { body }))
            .layer(axum::middleware::from_fn_with_state(
                TurnstileState::disabled(),
                middleware,
            ));
        let request = Request::builder()
            .method(Method::POST)
            .uri("/session")
            .body(Body::from("request body"))
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            to_bytes(response.into_body(), usize::MAX).await.unwrap(),
            "request body"
        );
    }

    #[tokio::test]
    async fn missing_token_is_forbidden() {
        let response = fixed_verification_app(Ok(TurnstileVerification::Accepted {
            action: Some("session-create".to_owned()),
            hostname: Some("example.com".to_owned()),
        }))
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn action_or_hostname_mismatch_is_forbidden() {
        for verification in [
            TurnstileVerification::Accepted {
                action: Some("temporary-answer".to_owned()),
                hostname: Some("example.com".to_owned()),
            },
            TurnstileVerification::Accepted {
                action: Some("session-create".to_owned()),
                hostname: Some("other.example.com".to_owned()),
            },
            TurnstileVerification::Accepted {
                action: Some("session-create".to_owned()),
                hostname: None,
            },
        ] {
            let response = fixed_verification_app(Ok(verification))
                .oneshot(
                    Request::builder()
                        .method(Method::POST)
                        .uri("/session")
                        .header(TURNSTILE_TOKEN_HEADER, HeaderValue::from_static("token"))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();

            assert_eq!(response.status(), StatusCode::FORBIDDEN);
        }
    }

    #[tokio::test]
    async fn verifier_unavailability_is_service_unavailable() {
        let response = fixed_verification_app(Err(TurnstileVerificationError::Unavailable))
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/session")
                    .header(TURNSTILE_TOKEN_HEADER, HeaderValue::from_static("token"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn rate_limit_runs_before_turnstile_and_handler() {
        let events = Arc::new(Mutex::new(Vec::new()));
        let rate_limit_state = RateLimitState::new(
            Arc::new(RecordingRateLimitStore {
                events: events.clone(),
            }),
            None,
        );
        let handler_events = events.clone();
        let app = Router::new()
            .route(
                "/session",
                post(move |body: String| {
                    let handler_events = handler_events.clone();
                    async move {
                        handler_events.lock().unwrap().push("handler");
                        body
                    }
                }),
            )
            // route_layer は後から追加した layer が外側になるため、rate limit を最後に追加する。
            .route_layer(axum::middleware::from_fn_with_state(
                state(events.clone()),
                middleware,
            ))
            .route_layer(axum::middleware::from_fn_with_state(
                rate_limit_state,
                rate_limit_middleware,
            ));
        let mut request = Request::builder()
            .method(Method::POST)
            .uri("/session")
            .header(TURNSTILE_TOKEN_HEADER, HeaderValue::from_static("token"))
            .body(Body::from("request body"))
            .unwrap();
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 8080))));

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            *events.lock().unwrap(),
            vec!["rate-limit", "turnstile", "handler"]
        );
    }
}
