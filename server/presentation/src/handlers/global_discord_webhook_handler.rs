use axum::{
    Extension, Json,
    extract::{State, rejection::JsonRejection},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use domain::{
    account::models::AccountUser, auth::Actor, global_discord_webhook::GlobalDiscordWebhookSetting,
};
use errors::ErrorExtra;
use resource::repository::RealInfrastructureRepository;
use types::non_empty_string::NonEmptyString;
use usecase::global_discord_webhook::GlobalDiscordWebhookUseCase;

use crate::{
    handlers::error_handler::handle_error,
    schemas::{
        error_responses::{BadRequest, Forbidden, InternalServerError, Unauthorized},
        global_discord_webhook::{
            GlobalDiscordWebhookStatusSchema, GlobalDiscordWebhookUpdateSchema,
        },
    },
};

#[utoipa::path(
    get,
    path = "/settings/global-discord-webhook",
    summary = "グローバル Discord Webhook 設定の取得",
    responses(
        (status = 200, body = GlobalDiscordWebhookStatusSchema),
        Unauthorized,
        Forbidden,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Settings"
)]
pub async fn get_global_discord_webhook(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
) -> Result<Json<GlobalDiscordWebhookStatusSchema>, Response> {
    let usecase = GlobalDiscordWebhookUseCase {
        repository: repository.global_discord_webhook_repository(),
    };
    let setting = usecase
        .get(&Actor::from(user))
        .await
        .map_err(handle_error)?;

    Ok(Json(GlobalDiscordWebhookStatusSchema {
        enabled: setting.enabled(),
    }))
}

#[utoipa::path(
    put,
    path = "/settings/global-discord-webhook",
    summary = "グローバル Discord Webhook 設定の更新",
    request_body = GlobalDiscordWebhookUpdateSchema,
    responses(
        (status = 204),
        BadRequest,
        Unauthorized,
        Forbidden,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Settings"
)]
pub async fn update_global_discord_webhook(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    json: Result<Json<GlobalDiscordWebhookUpdateSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let Json(request) = json.map_err_to_error().map_err(handle_error)?;
    let url = request
        .url
        .map(NonEmptyString::try_new)
        .transpose()
        .map_err(errors::Error::from)
        .map_err(handle_error)?;
    let setting = GlobalDiscordWebhookSetting::from_optional_url(url)
        .map_err(errors::Error::from)
        .map_err(handle_error)?;
    let usecase = GlobalDiscordWebhookUseCase {
        repository: repository.global_discord_webhook_repository(),
    };
    usecase
        .update(&Actor::from(user), setting)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::NO_CONTENT)
}
