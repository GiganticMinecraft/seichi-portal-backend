use serde::Serialize;

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct ErrorResponse {
    #[serde(rename = "errorCode")]
    pub error_code: String,
    pub reason: String,
}
