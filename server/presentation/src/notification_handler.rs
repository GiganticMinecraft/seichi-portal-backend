use axum::{extract::State, http::StatusCode, response::IntoResponse, Extension, Json};
use domain::{
    notification::models::NotificationSource, repository::Repositories, user::models::User,
};
use itertools::Itertools;
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::notification::NotificationUseCase;

use crate::schemas::notification::notification_response_schemas::NotificationResponse;

pub async fn fetch_by_recipient_id(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
) -> impl IntoResponse {
    let notification_usecase = NotificationUseCase {
        repository: repository.notification_repository(),
    };

    match notification_usecase.fetch_notifications(user.id).await {
        Ok(notifications) => {
            let notification_response = notifications
                .into_iter()
                .map(|notification| {
                    let notification_source = match notification.source() {
                        NotificationSource::Message(message_id) => {
                            ("Message".to_string(), message_id.to_string())
                        }
                    };

                    NotificationResponse {
                        id: notification.id().to_owned(),
                        source_type: notification_source.0,
                        source_id: notification_source.1,
                        is_read: notification.is_read().to_owned(),
                    }
                })
                .collect_vec();

            (StatusCode::OK, Json(json!(notification_response)))
        }
        Err(_err) => {
            // TODO: 今のエラーハンドリングでは非効率すぎるので、解決してから書く
            todo!()
        }
    }
}
