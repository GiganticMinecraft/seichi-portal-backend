use std::time::Duration;

use common::turnstile::{TurnstileVerification, TurnstileVerificationError, TurnstileVerifier};
use serde::{Deserialize, Serialize};

use crate::outgoing::http::HTTP_CLIENT;

pub const SITEVERIFY_URL: &str = "https://challenges.cloudflare.com/turnstile/v0/siteverify";
const SITEVERIFY_TIMEOUT: Duration = Duration::from_secs(3);

pub struct TurnstileSiteverifyClient {
    secret_key: String,
    siteverify_url: String,
}

impl TurnstileSiteverifyClient {
    pub fn new(secret_key: String) -> Self {
        Self::with_siteverify_url(secret_key, SITEVERIFY_URL)
    }

    /// Siteverify の送信先を差し替えられるため、外部 API 境界をローカルで検証できる。
    pub fn with_siteverify_url(secret_key: String, siteverify_url: impl Into<String>) -> Self {
        Self {
            secret_key,
            siteverify_url: siteverify_url.into(),
        }
    }
}

#[derive(Serialize)]
struct SiteverifyRequest<'a> {
    secret: &'a str,
    response: &'a str,
}

#[derive(Deserialize)]
struct SiteverifyResponse {
    success: bool,
    #[serde(default, rename = "error-codes")]
    error_codes: Vec<String>,
    action: Option<String>,
    hostname: Option<String>,
}

fn is_token_rejection(code: &str) -> bool {
    matches!(
        code,
        "missing-input-response" | "invalid-input-response" | "timeout-or-duplicate"
    )
}

fn is_service_failure(code: &str) -> bool {
    matches!(
        code,
        "missing-input-secret" | "invalid-input-secret" | "bad-request" | "internal-error"
    )
}

fn classify_siteverify_response(
    response: SiteverifyResponse,
) -> Result<TurnstileVerification, TurnstileVerificationError> {
    if response.success {
        return Ok(TurnstileVerification::Accepted {
            action: response.action,
            hostname: response.hostname,
        });
    }

    if response.error_codes.is_empty()
        || response
            .error_codes
            .iter()
            .any(|code| is_service_failure(code))
        || response
            .error_codes
            .iter()
            .any(|code| !is_token_rejection(code))
    {
        return Err(TurnstileVerificationError::Unavailable);
    }

    Ok(TurnstileVerification::Rejected)
}

#[async_trait::async_trait]
impl TurnstileVerifier for TurnstileSiteverifyClient {
    async fn verify(
        &self,
        token: &str,
    ) -> Result<TurnstileVerification, TurnstileVerificationError> {
        let response = HTTP_CLIENT
            .post(&self.siteverify_url)
            .timeout(SITEVERIFY_TIMEOUT)
            .json(&SiteverifyRequest {
                secret: &self.secret_key,
                response: token,
            })
            .send()
            .await
            .map_err(|_| TurnstileVerificationError::Unavailable)?
            .error_for_status()
            .map_err(|_| TurnstileVerificationError::Unavailable)?;

        let response = response
            .json::<SiteverifyResponse>()
            .await
            .map_err(|_| TurnstileVerificationError::InvalidResponse)?;

        classify_siteverify_response(response)
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use common::turnstile::TurnstileVerifier;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    use super::*;

    fn response(success: bool, error_codes: &[&str]) -> SiteverifyResponse {
        SiteverifyResponse {
            success,
            error_codes: error_codes.iter().map(|code| (*code).to_owned()).collect(),
            action: None,
            hostname: None,
        }
    }

    async fn spawn_siteverify_server(
        response_body: String,
    ) -> (String, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut chunk = [0_u8; 4096];
            let header_end = loop {
                let read = socket.read(&mut chunk).await.unwrap();
                assert!(read > 0);
                request.extend_from_slice(&chunk[..read]);
                if let Some(position) = request.windows(4).position(|window| window == b"\r\n\r\n")
                {
                    break position;
                }
            };

            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = headers
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
                .unwrap();
            let body_start = header_end + 4;
            while request.len() < body_start + content_length {
                let read = socket.read(&mut chunk).await.unwrap();
                assert!(read > 0);
                request.extend_from_slice(&chunk[..read]);
            }

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            socket.write_all(response.as_bytes()).await.unwrap();

            String::from_utf8(request[body_start..body_start + content_length].to_vec()).unwrap()
        });

        (format!("http://{address}"), task)
    }

    #[test]
    fn token_failures_are_rejected_without_becoming_service_errors() {
        for code in [
            "missing-input-response",
            "invalid-input-response",
            "timeout-or-duplicate",
        ] {
            assert_eq!(
                classify_siteverify_response(response(false, &[code])),
                Ok(TurnstileVerification::Rejected)
            );
        }
    }

    #[test]
    fn service_and_configuration_failures_are_unavailable() {
        for code in [
            "missing-input-secret",
            "invalid-input-secret",
            "bad-request",
            "internal-error",
            "unknown-error",
        ] {
            assert_eq!(
                classify_siteverify_response(response(false, &[code])),
                Err(TurnstileVerificationError::Unavailable)
            );
        }
    }

    #[test]
    fn mixed_token_and_unknown_failures_are_unavailable() {
        assert_eq!(
            classify_siteverify_response(response(
                false,
                &["invalid-input-response", "unknown-error"]
            )),
            Err(TurnstileVerificationError::Unavailable)
        );
    }

    #[tokio::test]
    async fn siteverify_sends_only_secret_and_response() {
        let (endpoint, request_task) = spawn_siteverify_server(
            r#"{"success":true,"action":"session-create","hostname":"example.com"}"#.to_owned(),
        )
        .await;
        let client = TurnstileSiteverifyClient::with_siteverify_url("secret".to_owned(), endpoint);

        assert_eq!(
            client.verify("token").await,
            Ok(TurnstileVerification::Accepted {
                action: Some("session-create".to_owned()),
                hostname: Some("example.com".to_owned()),
            })
        );

        let request_body = request_task.await.unwrap();
        assert!(request_body.contains(r#""secret":"secret""#));
        assert!(request_body.contains(r#""response":"token""#));
        assert!(!request_body.contains("remoteip"));
    }
}
