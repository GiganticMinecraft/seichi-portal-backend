use axum::{http::StatusCode, response::IntoResponse, Extension, Json};
use domain::user::models::User;
use serde_json::json;

pub async fn get_my_user_info(Extension(user): Extension<User>) -> impl IntoResponse {
    (StatusCode::OK, Json(json!(user))).into_response()
}
