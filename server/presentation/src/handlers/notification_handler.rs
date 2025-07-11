use axum::{
    Extension, Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use domain::{repository::Repositories, user::models::User};
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::notification::NotificationUseCase;
use uuid::Uuid;

use crate::{
    handlers::error_handler::{handle_error, handle_json_rejection},
    schemas::notification::{
        notification_request_schemas::NotificationSettingsUpdateSchema,
        notification_response_schemas::NotificationSettingsResponse,
    },
};

pub async fn get_notification_settings(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path(target_user_id): Path<Uuid>,
) -> impl IntoResponse {
    let notification_usecase = NotificationUseCase {
        repository: repository.notification_repository(),
        user_repository: repository.user_repository(),
    };

    match notification_usecase
        .fetch_notification_settings(user, target_user_id)
        .await
    {
        Ok(settings) => {
            let response = NotificationSettingsResponse {
                is_send_message_notification: *settings.is_send_message_notification(),
            };

            (StatusCode::OK, Json(json!(response))).into_response()
        }
        Err(err) => handle_error(err).into_response(),
    }
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

    let Json(notification_settings) = json.map_err(handle_json_rejection)?;

    Ok(
        match notification_usecase
            .update_notification_settings(&user, notification_settings.is_send_message_notification)
            .await
        {
            Ok(_) => StatusCode::OK.into_response(),
            Err(err) => handle_error(err).into_response(),
        },
    )
}
