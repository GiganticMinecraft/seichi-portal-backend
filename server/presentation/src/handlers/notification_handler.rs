use crate::schemas::error_responses::*;
use crate::{
    handlers::error_handler::handle_error,
    schemas::notification::{
        notification_request_schemas::{NotificationListQuery, NotificationSettingsUpdateSchema},
        notification_response_schemas::{
            NotificationPageResponse, NotificationResponse, NotificationSettingsResponse,
        },
    },
};
use axum::extract::rejection::{PathRejection, QueryRejection};
use axum::{
    Extension, Json,
    extract::{Path, Query, State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use domain::{
    account::models::AccountUser,
    notification::models::NotificationPagePosition,
    pagination::{PageLimit, PageRequest},
    repository::Repositories,
};
use errors::{Error, ErrorExtra, presentation::PresentationError};
use resource::repository::RealInfrastructureRepository;
use usecase::notification::NotificationUseCase;
use uuid::Uuid;

#[derive(serde::Deserialize, serde::Serialize)]
struct NotificationCursor {
    after_notification_id: Uuid,
}

fn notification_page_request(
    query: NotificationListQuery,
) -> Result<PageRequest<NotificationPagePosition>, Error> {
    let limit = match query.limit {
        Some(limit) => PageLimit::try_new(limit).map_err(|error| {
            Error::from(PresentationError::QueryRejection {
                cause: format!("Invalid limit: {}.", error.value()),
            })
        })?,
        None => PageLimit::default_limit(),
    };
    let after = query
        .cursor
        .as_deref()
        .map(|cursor| {
            let decoded = URL_SAFE_NO_PAD.decode(cursor).map_err(|_| {
                Error::from(PresentationError::QueryRejection {
                    cause: "Invalid cursor.".to_string(),
                })
            })?;
            let cursor: NotificationCursor = serde_json::from_slice(&decoded).map_err(|_| {
                Error::from(PresentationError::QueryRejection {
                    cause: "Invalid cursor.".to_string(),
                })
            })?;
            Ok::<_, Error>(NotificationPagePosition::new(
                cursor.after_notification_id.into(),
            ))
        })
        .transpose()?;

    Ok(PageRequest::new(after, limit))
}

fn encode_notification_cursor(position: NotificationPagePosition) -> Result<String, Error> {
    let bytes = serde_json::to_vec(&NotificationCursor {
        after_notification_id: position.id().into_inner(),
    })
    .map_err(|_| {
        Error::from(PresentationError::QueryRejection {
            cause: "Invalid cursor.".to_string(),
        })
    })?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

#[derive(utoipa::IntoResponses)]
pub enum GetNotificationsResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(NotificationPageResponse),
}

impl IntoResponse for GetNotificationsResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

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
    path = "/notifications",
    summary = "自身の通知一覧を取得する",
    params(NotificationListQuery),
    responses(
        GetNotificationsResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Notifications"
)]
pub async fn get_notifications(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    query: Result<Query<NotificationListQuery>, QueryRejection>,
) -> Result<GetNotificationsResponse, Response> {
    let notification_usecase = NotificationUseCase {
        repository: repository.notification_repository(),
        user_repository: repository.user_repository(),
    };
    let Query(query) = query.map_err_to_error().map_err(handle_error)?;
    let request = notification_page_request(query).map_err(handle_error)?;
    let page = notification_usecase
        .fetch_notifications(&user, request)
        .await
        .map_err(handle_error)?;
    let (notifications, next) = page.into_parts();

    Ok(GetNotificationsResponse::Ok(NotificationPageResponse {
        items: notifications
            .into_iter()
            .map(NotificationResponse::from)
            .collect(),
        next_cursor: next
            .map(encode_notification_cursor)
            .transpose()
            .map_err(handle_error)?,
    }))
}

#[utoipa::path(
    patch,
    path = "/notifications/{notification_id}/read",
    summary = "通知を既読にする",
    params(("notification_id" = String, Path, description = "Notification UUID")),
    responses(
        (status = 204, description = "There is no content to send for this request, but the headers may be useful."),
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Notifications"
)]
pub async fn mark_notification_as_read(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Result<impl IntoResponse, Response> {
    let notification_usecase = NotificationUseCase {
        repository: repository.notification_repository(),
        user_repository: repository.user_repository(),
    };
    let Path(notification_id) = path.map_err_to_error().map_err(handle_error)?;

    notification_usecase
        .mark_notification_as_read(&user, notification_id.into())
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

#[utoipa::path(
    patch,
    path = "/notifications/read-all",
    summary = "すべての通知を既読にする",
    responses(
        (status = 204, description = "There is no content to send for this request, but the headers may be useful."),
        Unauthorized,
        Forbidden,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Notifications"
)]
pub async fn mark_all_notifications_as_read(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
) -> Result<impl IntoResponse, Response> {
    let notification_usecase = NotificationUseCase {
        repository: repository.notification_repository(),
        user_repository: repository.user_repository(),
    };

    notification_usecase
        .mark_all_notifications_as_read(&user)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::NO_CONTENT.into_response())
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
    Extension(user): Extension<AccountUser>,
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
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
) -> Result<GetNotificationSettingsResponse, Response> {
    let notification_usecase = NotificationUseCase {
        repository: repository.notification_repository(),
        user_repository: repository.user_repository(),
    };

    let user_id = user.id().into_inner();

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
    Extension(user): Extension<AccountUser>,
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
