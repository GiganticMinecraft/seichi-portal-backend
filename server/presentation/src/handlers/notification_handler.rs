use crate::{
    handlers::error_handler::handle_error,
    schemas::notification::{
        notification_request_schemas::NotificationSettingsUpdateSchema,
        notification_response_schemas::NotificationSettingsResponse,
    },
};
use axum::extract::rejection::PathRejection;
use axum::{
    Extension, Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use domain::{repository::Repositories, user::models::User};
use errors::ErrorExtra;
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::notification::NotificationUseCase;
use uuid::Uuid;

pub async fn get_notification_settings(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Result<impl IntoResponse, Response> {
    let notification_usecase = NotificationUseCase {
        repository: repository.notification_repository(),
        user_repository: repository.user_repository(),
    };

    let Path(target_user_id) = path.map_err_to_error().map_err(handle_error)?;

    let settings = notification_usecase
        .fetch_notification_settings(user, target_user_id)
        .await
        .map_err(handle_error)?;

    let response = NotificationSettingsResponse {
        is_send_message_notification: *settings.is_send_message_notification(),
    };

    Ok((StatusCode::OK, Json(json!(response))).into_response())
}

pub async fn update_notification_settings(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    json: Result<Json<NotificationSettingsUpdateSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let notification_usecase = NotificationUseCase {
        repository: repository.notification_repository(),
        user_repository: repository.user_repository(),
    };

    let Json(notification_settings) = json.map_err_to_error().map_err(handle_error)?;

    notification_usecase
        .update_notification_settings(&user, notification_settings.is_send_message_notification)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::OK.into_response())
}
