use axum::{extract::State, http::StatusCode, response::IntoResponse, Extension, Json};
use domain::{
    notification::models::NotificationSource, repository::Repositories, user::models::User,
};
use itertools::Itertools;
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::notification::NotificationUseCase;

use crate::{
    error_handler::handle_error,
    schemas::notification::{
        notification_request_schemas::NotificationUpdateReadStateSchema,
        notification_response_schemas::NotificationResponse,
    },
};

pub async fn fetch_by_recipient_id(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
) -> impl IntoResponse {
    let notification_usecase = NotificationUseCase {
        repository: repository.notification_repository(),
    };

    let notification_response_or_error = notification_usecase
        .fetch_notifications(user.id)
        .await
        .and_then(|notifications| {
            notifications
                .into_iter()
                .map(|notification| {
                    notification.try_read(&user).map(|notification| {
                        let (source_type, source_id) = match notification.source() {
                            NotificationSource::Message(message_id) => {
                                ("MESSAGE".to_string(), message_id.to_string())
                            }
                        };

                        NotificationResponse {
                            id: notification.id().to_owned(),
                            source_type,
                            source_id,
                            is_read: notification.is_read().to_owned(),
                        }
                    })
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
                    notification.try_read(&user).map(|notification| {
                        let (source_type, source_id) = match notification.source() {
                            NotificationSource::Message(message_id) => {
                                ("MESSAGE".to_string(), message_id.to_string())
                            }
                        };

                        NotificationResponse {
                            id: notification.id().to_owned(),
                            source_type,
                            source_id,
                            is_read: notification.is_read().to_owned(),
                        }
                    })
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
