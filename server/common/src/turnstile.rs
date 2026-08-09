use async_trait::async_trait;

/// Siteverify のレスポンスを、外部 API の DTO ではなく検証結果として表す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnstileVerification {
    Accepted {
        action: Option<String>,
        hostname: Option<String>,
    },
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnstileVerificationError {
    Unavailable,
    InvalidResponse,
}

#[async_trait]
pub trait TurnstileVerifier: Send + Sync {
    async fn verify(
        &self,
        token: &str,
    ) -> Result<TurnstileVerification, TurnstileVerificationError>;
}
