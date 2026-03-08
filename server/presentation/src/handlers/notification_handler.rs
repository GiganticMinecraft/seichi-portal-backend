use crate::schemas::error_responses::*;
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
use usecase::notification::NotificationUseCase;
use uuid::Uuid;

#[derive(utoipa::IntoResponses)]
pub enum GetNotificationSettingsResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(NotificationSettingsResponse),
}

impl IntoResponse for GetNotificationSettingsResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

#[utoipa::path(
    get,
    path = "/notifications/settings/{uuid}",
    summary = "通知の設定を取得する",
    params(
        ("uuid" = String, Path, description = "User UUID"),
    ),
    responses(
        GetNotificationSettingsResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Notifications"
)]
pub async fn get_notification_settings(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Result<GetNotificationSettingsResponse, Response> {
    let notification_usecase = NotificationUseCase {
        repository: repository.notification_repository(),
        user_repository: repository.user_repository(),
    };

    let Path(target_user_id) = path.map_err_to_error().map_err(handle_error)?;

    let settings = notification_usecase
        .fetch_notification_settings(user, target_user_id)
        .await
        .map_err(handle_error)?;

    Ok(GetNotificationSettingsResponse::Ok(
        NotificationSettingsResponse {
            is_send_message_notification: *settings.is_send_message_notification(),
        },
    ))
}

#[utoipa::path(
    get,
    path = "/notifications/settings/me",
    summary = "自身の通知設定の取得",
    responses(
        GetNotificationSettingsResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Notifications"
)]
pub async fn get_my_notification_settings(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
) -> Result<GetNotificationSettingsResponse, Response> {
    let notification_usecase = NotificationUseCase {
        repository: repository.notification_repository(),
        user_repository: repository.user_repository(),
    };

    let user_id = user.id.to_owned();

    let settings = notification_usecase
        .fetch_notification_settings(user, user_id)
        .await
        .map_err(handle_error)?;

    Ok(GetNotificationSettingsResponse::Ok(
        NotificationSettingsResponse {
            is_send_message_notification: *settings.is_send_message_notification(),
        },
    ))
}

#[utoipa::path(
    patch,
    path = "/notifications/settings/me",
    summary = "通知設定の更新",
    request_body = NotificationSettingsUpdateSchema,
    responses(
        (status = 200, description = "The request has succeeded."),
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Notifications"
)]
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
