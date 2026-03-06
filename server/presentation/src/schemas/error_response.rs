use serde::Serialize;

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct ErrorResponse {
    #[serde(rename = "type")]
    pub problem_type: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
    #[serde(rename = "errorCode")]
    pub error_code: String,
}
