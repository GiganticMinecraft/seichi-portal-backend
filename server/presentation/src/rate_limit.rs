use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use crate::schemas::error_response::ErrorResponse;
use axum::{
    body::Body,
    extract::{ConnectInfo, State},
    http::{HeaderMap, HeaderValue, Method, Request, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use common::rate_limit::{RateLimitDecision, RateLimitQuota, RateLimitStore, RateLimitStoreError};
use domain::{account::models::AccountUser, auth::Actor};
use uuid::Uuid;

const API_PREFIX: &str = "/api/v1";
const ANONYMOUS_GET_LIMIT: u64 = 60;
const AUTHENTICATED_GET_LIMIT: u64 = 600;
const AUTHENTICATED_WRITE_LIMIT: u64 = 120;
const TEMPORARY_IP_HOURLY_LIMIT: u64 = 30;
const TEMPORARY_FORM_HOURLY_LIMIT: u64 = 10;
const TEMPORARY_IP_BURST_LIMIT: u64 = 5;
const SESSION_CREATE_IP_HOURLY_LIMIT: u64 = 10;
const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;

pub const PROXY_SECRET_HEADER: &str = "x-seichi-proxy-secret";
pub const CLIENT_IP_HEADER: &str = "x-seichi-client-ip";

#[derive(Clone)]
pub struct RateLimitState {
    pub store: Arc<dyn RateLimitStore>,
    pub proxy_secret: Option<String>,
}

impl RateLimitState {
    pub fn new(store: Arc<dyn RateLimitStore>, proxy_secret: Option<String>) -> Self {
        Self {
            store,
            proxy_secret: proxy_secret.filter(|secret| !secret.is_empty()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitIdentity {
    User(String),
    Ip(IpAddr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientIpResolution {
    Canonical(IpAddr),
    TcpPeer(IpAddr),
    Unavailable,
}

/// Resolve the address that may be used for an anonymous quota.  A proxy
/// address is trusted only when both the configured shared secret and a
/// syntactically valid client IP are present.
pub fn resolve_client_ip(
    headers: &HeaderMap,
    tcp_peer: Option<IpAddr>,
    proxy_secret: Option<&str>,
) -> ClientIpResolution {
    let trusted_proxy = proxy_secret.is_some_and(|expected| {
        headers
            .get(PROXY_SECRET_HEADER)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|actual| actual == expected)
    });

    if trusted_proxy
        && let Some(ip) = headers
            .get(CLIENT_IP_HEADER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<IpAddr>().ok())
    {
        return ClientIpResolution::Canonical(ip);
    }

    tcp_peer
        .map(ClientIpResolution::TcpPeer)
        .unwrap_or(ClientIpResolution::Unavailable)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitPlan {
    AnonymousGet { ip: IpAddr },
    AuthenticatedGet { user_id: String },
    AuthenticatedWrite { user_id: String },
    TemporaryAnswer { ip: IpAddr, form_id: Uuid },
    SessionCreate { ip: IpAddr },
    Skip,
}

impl RateLimitPlan {
    fn label(&self) -> &'static str {
        match self {
            Self::AnonymousGet { .. } => "anonymous_get",
            Self::AuthenticatedGet { .. } => "authenticated_get",
            Self::AuthenticatedWrite { .. } => "authenticated_write",
            Self::TemporaryAnswer { .. } => "temporary_answer",
            Self::SessionCreate { .. } => "session_create",
            Self::Skip => "skip",
        }
    }

    fn quotas(&self) -> Vec<RateLimitQuota> {
        match self {
            Self::AnonymousGet { ip } => {
                vec![RateLimitQuota::new(
                    format!("rl:v1:anon:get:{ip}"),
                    ANONYMOUS_GET_LIMIT,
                    MINUTE,
                )]
            }
            Self::AuthenticatedGet { user_id } => {
                vec![RateLimitQuota::new(
                    format!("rl:v1:user:get:{user_id}"),
                    AUTHENTICATED_GET_LIMIT,
                    MINUTE,
                )]
            }
            Self::AuthenticatedWrite { user_id } => {
                vec![RateLimitQuota::new(
                    format!("rl:v1:user:write:{user_id}"),
                    AUTHENTICATED_WRITE_LIMIT,
                    MINUTE,
                )]
            }
            Self::TemporaryAnswer { ip, form_id } => vec![
                RateLimitQuota::new(
                    format!("rl:v1:temporary:ip:{ip}"),
                    TEMPORARY_IP_HOURLY_LIMIT,
                    HOUR,
                ),
                RateLimitQuota::new(
                    format!("rl:v1:temporary:form:{ip}:{form_id}"),
                    TEMPORARY_FORM_HOURLY_LIMIT,
                    HOUR,
                ),
                RateLimitQuota::new(
                    format!("rl:v1:temporary:burst:{ip}"),
                    TEMPORARY_IP_BURST_LIMIT,
                    10 * MINUTE,
                ),
            ],
            Self::SessionCreate { ip } => vec![RateLimitQuota::new(
                format!("rl:v1:session:create:{ip}"),
                SESSION_CREATE_IP_HOURLY_LIMIT,
                HOUR,
            )],
            Self::Skip => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitPolicyError {
    MissingAuthenticatedIdentity,
}

/// Pure route policy calculation.  The middleware deliberately receives the
/// identity only after auth/optional-auth has run, so invalid credentials keep
/// their existing 401 response and never consume a user quota.
pub fn policy_for(
    method: &Method,
    path: &str,
    identity: Option<&RateLimitIdentity>,
) -> Result<RateLimitPlan, RateLimitPolicyError> {
    if *method == Method::OPTIONS {
        return Ok(RateLimitPlan::Skip);
    }

    // `Router::nest` strips its prefix from the request URI before invoking a
    // route-layer middleware.  Unit tests and callers outside that nest may
    // still provide the full versioned path, so accept both representations.
    let path = if path == API_PREFIX {
        "/"
    } else if let Some(stripped) = path.strip_prefix(API_PREFIX) {
        if stripped.starts_with('/') {
            stripped
        } else {
            // Do not treat a similarly-prefixed endpoint such as `/api/v10`
            // as part of this API.
            return Ok(RateLimitPlan::Skip);
        }
    } else if path.starts_with('/') {
        path
    } else {
        return Ok(RateLimitPlan::Skip);
    };
    let segments = path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    if *method == Method::GET
        && segments.first() == Some(&"forms")
        && (segments.len() == 1 || segments.len() == 2)
    {
        return Ok(match identity {
            Some(RateLimitIdentity::User(user_id)) => RateLimitPlan::AuthenticatedGet {
                user_id: user_id.clone(),
            },
            Some(RateLimitIdentity::Ip(ip)) => RateLimitPlan::AnonymousGet { ip: *ip },
            None => RateLimitPlan::Skip,
        });
    }

    if *method == Method::POST
        && segments.len() == 3
        && segments[0] == "forms"
        && segments[2] == "temporary-answers"
    {
        return Ok(match identity {
            Some(RateLimitIdentity::Ip(ip)) => RateLimitPlan::TemporaryAnswer {
                ip: *ip,
                form_id: match Uuid::parse_str(segments[1]) {
                    Ok(form_id) => form_id,
                    Err(_) => return Ok(RateLimitPlan::Skip),
                },
            },
            _ => RateLimitPlan::Skip,
        });
    }

    if segments == ["session"] {
        return match *method {
            Method::POST => Ok(match identity {
                Some(RateLimitIdentity::Ip(ip)) => RateLimitPlan::SessionCreate { ip: *ip },
                _ => RateLimitPlan::Skip,
            }),
            Method::DELETE => match identity {
                Some(RateLimitIdentity::User(user_id)) => Ok(RateLimitPlan::AuthenticatedWrite {
                    user_id: user_id.clone(),
                }),
                _ => Err(RateLimitPolicyError::MissingAuthenticatedIdentity),
            },
            _ => Ok(RateLimitPlan::Skip),
        };
    }

    if let Some(RateLimitIdentity::User(user_id)) = identity {
        return Ok(if *method == Method::GET {
            RateLimitPlan::AuthenticatedGet {
                user_id: user_id.clone(),
            }
        } else {
            RateLimitPlan::AuthenticatedWrite {
                user_id: user_id.clone(),
            }
        });
    }

    Ok(RateLimitPlan::Skip)
}

fn identity_from_request(
    request: &Request<Body>,
    proxy_secret: Option<&str>,
) -> (Option<RateLimitIdentity>, bool) {
    if let Some(user) = request.extensions().get::<AccountUser>() {
        return (Some(RateLimitIdentity::User(user.id().to_string())), false);
    }
    if let Some(actor) = request.extensions().get::<Actor>()
        && let Actor::AccountUser(user) = actor
    {
        return (Some(RateLimitIdentity::User(user.id().to_string())), false);
    }

    let tcp_peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|connect_info| connect_info.0.ip());
    match resolve_client_ip(request.headers(), tcp_peer, proxy_secret) {
        ClientIpResolution::Canonical(ip) | ClientIpResolution::TcpPeer(ip) => {
            (Some(RateLimitIdentity::Ip(ip)), false)
        }
        ClientIpResolution::Unavailable => (None, true),
    }
}

fn problem_response(status: StatusCode, detail: &str, error_code: &str) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "application/problem+json")],
        axum::Json(ErrorResponse {
            problem_type: "about:blank".to_owned(),
            title: if status == StatusCode::TOO_MANY_REQUESTS {
                "Too Many Requests".to_owned()
            } else {
                "Internal Server Error".to_owned()
            },
            status: status.as_u16(),
            detail: detail.to_owned(),
            error_code: error_code.to_owned(),
            restriction: None,
        }),
    )
        .into_response()
}

fn set_rate_limit_headers(response: &mut Response, decision: RateLimitDecision) {
    let retry_after = decision.retry_after_seconds.max(1);
    let reset = if decision.allowed {
        decision.reset_seconds.max(1)
    } else {
        retry_after
    };
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(&decision.limit.to_string()) {
        headers.insert("ratelimit-limit", value);
    }
    if let Ok(value) = HeaderValue::from_str(&decision.remaining.to_string()) {
        headers.insert("ratelimit-remaining", value);
    }
    if let Ok(value) = HeaderValue::from_str(&reset.to_string()) {
        headers.insert("ratelimit-reset", value);
    }
    if !decision.allowed
        && let Ok(value) = HeaderValue::from_str(&retry_after.to_string())
    {
        headers.insert(header::RETRY_AFTER, value);
    }
}

pub async fn middleware(
    State(state): State<RateLimitState>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let (identity, ip_unavailable) = identity_from_request(&request, state.proxy_secret.as_deref());
    let plan = match policy_for(request.method(), request.uri().path(), identity.as_ref()) {
        Ok(plan) => plan,
        Err(RateLimitPolicyError::MissingAuthenticatedIdentity) => {
            tracing::error!("rate-limit route requires an authenticated identity");
            return problem_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Rate-limit identity could not be resolved.",
                "INTERNAL_SERVER_ERROR",
            );
        }
    };

    if ip_unavailable {
        tracing::warn!(
            plan = plan.label(),
            "rate limiting skipped because client IP is unavailable"
        );
    }

    if matches!(plan, RateLimitPlan::Skip) {
        return next.run(request).await;
    }

    let label = plan.label();
    let quotas = plan.quotas();
    let decision = match state.store.check(&quotas).await {
        Ok(decision) => decision,
        Err(RateLimitStoreError::Unavailable(error)) => {
            tracing::warn!(plan = label, error = %error, "Valkey unavailable; rate limiting failed open");
            return next.run(request).await;
        }
        Err(RateLimitStoreError::Invalid(error)) => {
            tracing::error!(plan = label, error = %error, "invalid rate-limit store response");
            return problem_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Rate-limit store returned an invalid response.",
                "INTERNAL_SERVER_ERROR",
            );
        }
    };

    if !decision.allowed {
        tracing::warn!(plan = label, "rate limit exceeded");
        let mut response = problem_response(
            StatusCode::TOO_MANY_REQUESTS,
            "Request rate limit exceeded.",
            "RATE_LIMIT_EXCEEDED",
        );
        set_rate_limit_headers(&mut response, decision);
        return response;
    }

    let mut response = next.run(request).await;
    set_rate_limit_headers(&mut response, decision);
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;

    fn ip(value: &str) -> IpAddr {
        value.parse().unwrap()
    }

    #[test]
    fn proxy_ip_requires_matching_secret() {
        let mut headers = HeaderMap::new();
        headers.insert(PROXY_SECRET_HEADER, HeaderValue::from_static("secret"));
        headers.insert(CLIENT_IP_HEADER, HeaderValue::from_static("203.0.113.4"));

        assert_eq!(
            resolve_client_ip(&headers, Some(ip("192.0.2.1")), Some("secret")),
            ClientIpResolution::Canonical(ip("203.0.113.4"))
        );
        assert_eq!(
            resolve_client_ip(&headers, Some(ip("192.0.2.1")), Some("wrong")),
            ClientIpResolution::TcpPeer(ip("192.0.2.1"))
        );
        assert_eq!(
            resolve_client_ip(&headers, Some(ip("192.0.2.1")), None),
            ClientIpResolution::TcpPeer(ip("192.0.2.1"))
        );
    }

    #[test]
    fn malformed_proxy_ip_uses_tcp_peer() {
        let mut headers = HeaderMap::new();
        headers.insert(PROXY_SECRET_HEADER, HeaderValue::from_static("secret"));
        headers.insert(CLIENT_IP_HEADER, HeaderValue::from_static("not-an-ip"));
        assert_eq!(
            resolve_client_ip(&headers, Some(ip("192.0.2.1")), Some("secret")),
            ClientIpResolution::TcpPeer(ip("192.0.2.1"))
        );
    }

    #[test]
    fn temporary_answer_has_only_temporary_quotas() {
        let form_id = "018F4F37-2F5E-7B9A-8B39-9A2F2695D7AD";
        let canonical_form_id = form_id.to_ascii_lowercase();
        let plan = policy_for(
            &Method::POST,
            &format!("/api/v1/forms/{form_id}/temporary-answers"),
            Some(&RateLimitIdentity::Ip(ip("192.0.2.1"))),
        )
        .unwrap();
        assert_eq!(plan.label(), "temporary_answer");
        assert_eq!(plan.quotas().len(), 3);
        assert!(
            plan.quotas()
                .iter()
                .any(|quota| quota.key.contains(&canonical_form_id))
        );
    }

    #[test]
    fn nested_router_stripped_form_path_still_gets_anonymous_quota() {
        let plan = policy_for(
            &Method::GET,
            "/forms",
            Some(&RateLimitIdentity::Ip(ip("192.0.2.1"))),
        )
        .unwrap();
        assert_eq!(plan.label(), "anonymous_get");
    }

    #[test]
    fn nested_router_stripped_temporary_path_still_gets_temporary_quota() {
        let plan = policy_for(
            &Method::POST,
            "/forms/018f4f37-2f5e-7b9a-8b39-9a2f2695d7ad/temporary-answers",
            Some(&RateLimitIdentity::Ip(ip("192.0.2.1"))),
        )
        .unwrap();
        assert_eq!(plan.label(), "temporary_answer");
    }

    #[test]
    fn nested_router_stripped_authenticated_path_uses_user_quota() {
        let plan = policy_for(
            &Method::GET,
            "/users",
            Some(&RateLimitIdentity::User("user-id".to_owned())),
        )
        .unwrap();
        assert_eq!(plan.label(), "authenticated_get");
    }

    #[test]
    fn invalid_temporary_form_id_does_not_create_a_key() {
        let plan = policy_for(
            &Method::POST,
            "/api/v1/forms/not-a-uuid/temporary-answers",
            Some(&RateLimitIdentity::Ip(ip("192.0.2.1"))),
        )
        .unwrap();
        assert_eq!(plan, RateLimitPlan::Skip);
    }

    #[test]
    fn authenticated_delete_session_uses_user_quota() {
        let plan = policy_for(
            &Method::DELETE,
            "/api/v1/session",
            Some(&RateLimitIdentity::User("user-id".to_owned())),
        )
        .unwrap();
        assert_eq!(plan.label(), "authenticated_write");
    }

    #[test]
    fn delete_session_without_user_is_an_error() {
        assert_eq!(
            policy_for(&Method::DELETE, "/api/v1/session", None),
            Err(RateLimitPolicyError::MissingAuthenticatedIdentity)
        );
    }
}
