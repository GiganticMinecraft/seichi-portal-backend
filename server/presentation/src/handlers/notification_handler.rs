use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use domain::{repository::Repositories, user::models::User};
use itertools::Itertools;
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::notification::NotificationUseCase;
use uuid::Uuid;

use crate::{
    handlers::error_handler::handle_error,
    schemas::notification::{
        notification_request_schemas::{
            NotificationSettingsUpdateSchema, NotificationUpdateReadStateSchema,
        },
        notification_response_schemas::{NotificationResponse, NotificationSettingsResponse},
    },
};

pub async fn fetch_by_request_user(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
) -> impl IntoResponse {
    let notification_usecase = NotificationUseCase {
        repository: repository.notification_repository(),
        user_repository: repository.user_repository(),
    };

    let notification_response_or_error = notification_usecase
        .fetch_notifications(user.id)
        .await
        .and_then(|notifications| {
            notifications
                .into_iter()
                .map(|notification| {
                    notification
                        .try_into_read(&user)
                        .map(Into::<NotificationResponse>::into)
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(Into::into)
        });

    match notification_response_or_error {
        Ok(notification_response) => {
            (StatusCode::OK, Json(json!(notification_response))).into_response()
        }
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn update_read_state(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Json(update_targets): Json<Vec<NotificationUpdateReadStateSchema>>,
) -> impl IntoResponse {
    let notification_usecase = NotificationUseCase {
        repository: repository.notification_repository(),
        user_repository: repository.user_repository(),
    };

    let update_targets = update_targets
        .into_iter()
        .map(|update_target| (update_target.notification_id, update_target.is_read))
        .collect_vec();

    let notification_response_or_error = notification_usecase
        .update_notification_read_status(&user, update_targets)
        .await
        .and_then(|updated_notifications| {
            updated_notifications
                .into_iter()
                .map(|notification| {
                    notification
                        .try_into_read(&user)
                        .map(Into::<NotificationResponse>::into)
                })
                .collect::<Result<Vec<_>, _>>()
                .map_err(Into::into)
        });

    match notification_response_or_error {
        Ok(notification_response) => {
            (StatusCode::OK, Json(json!(notification_response))).into_response()
        }
        Err(err) => handle_error(err).into_response(),
    }
}

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
    Json(notification_settings): Json<NotificationSettingsUpdateSchema>,
) -> impl IntoResponse {
    let notification_usecase = NotificationUseCase {
        repository: repository.notification_repository(),
        user_repository: repository.user_repository(),
    };

    match notification_usecase
        .update_notification_settings(&user, notification_settings.is_send_message_notification)
        .await
    {
        Ok(_) => StatusCode::NO_CONTENT.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}
