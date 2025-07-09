use axum::{
    Extension, Json,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::IntoResponse,
};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};
use domain::{
    repository::Repositories,
    user::models::{User, UserSessionExpires},
};
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::user::UserUseCase;
use uuid::Uuid;

use crate::schemas::user::UserUpdateSchema;
use crate::{handlers::error_handler::handle_error, schemas::user::DiscordOAuthToken};
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::response::Response;
use axum_extra::typed_header::TypedHeaderRejection;
use errors::presentation::PresentationError;
use errors::{Error, ErrorExtra};

pub async fn get_my_user_info(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
) -> Result<impl IntoResponse, Response> {
    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };

    let user_dto = user_use_case
        .fetch_user_information(&user, user.id)
        .await
        .map_err(handle_error)?;
    let discord_user_id_with_name = user_dto.discord_user.map(|user| {
        (
            user.id().to_owned().into_inner(),
            user.name().to_owned().into_inner(),
        )
    });
    Ok((
        StatusCode::OK,
        Json(json!({
            "id": user_dto.user.id.to_string(),
            "name": user_dto.user.name,
            "role": user_dto.user.role.to_string(),
            "discord_user_id": discord_user_id_with_name.to_owned().map(|(discord_user, _)| discord_user),
            "discord_username": discord_user_id_with_name.map(|(_, username)| username),
        })),
    ).into_response())
}

pub async fn get_user_info(
    Extension(actor): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Result<impl IntoResponse, Response> {
    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };

    let Path(uuid) = path.map_err_to_error().map_err(handle_error)?;

    let user_dto = user_use_case
        .fetch_user_information(&actor, uuid)
        .await
        .map_err(handle_error)?;
    let discord_user_id_with_name = user_dto.discord_user.map(|user| {
        (
            user.id().to_owned().into_inner(),
            user.name().to_owned().into_inner(),
        )
    });
    Ok((
        StatusCode::OK,
        Json(json!({
            "id": user_dto.user.id.to_string(),
            "name": user_dto.user.name,
            "role": user_dto.user.role.to_string(),
            "discord_user_id": discord_user_id_with_name.to_owned().map(|(discord_user, _)| discord_user),
            "discord_username": discord_user_id_with_name.map(|(_, username)| username),
        })),
    ).into_response())
}

pub async fn patch_user_role(
    Extension(actor): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<Uuid>, PathRejection>,
    json: Result<Json<UserUpdateSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };

    let Path(uuid) = path.map_err_to_error().map_err(handle_error)?;
    let Json(user) = json.map_err_to_error().map_err(handle_error)?;

    let user = if let Some(role) = user.role {
        user_use_case.patch_user_role(&actor, uuid, role).await
    } else {
        user_use_case.find_by(&actor, uuid).await
    }
    .map_err(handle_error)?;

    Ok((StatusCode::OK, Json(user)).into_response())
}

pub async fn user_list(
    Extension(actor): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
) -> Result<impl IntoResponse, Response> {
    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };

    let users = user_use_case
        .fetch_all_users(&actor)
        .await
        .map_err(handle_error)?;
    Ok((StatusCode::OK, Json(json!(users))).into_response())
}

pub async fn start_session(
    State(repository): State<RealInfrastructureRepository>,
    header: Result<TypedHeader<Authorization<Bearer>>, TypedHeaderRejection>,
    Json(expires): Json<UserSessionExpires>,
) -> Result<impl IntoResponse, Response> {
    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };

    let TypedHeader(auth) = header.map_err_to_error().map_err(handle_error)?;

    let token = auth.token();
    match user_use_case
        .fetch_user_by_xbox_token(token.to_string())
        .await
    {
        Ok(Some(user)) => {
            let expires = expires.expires;
            let session_id = user_use_case
                .start_user_session(token.to_string(), &user, expires)
                .await
                .map_err(handle_error)?;
            Ok((StatusCode::OK, [(
                header::SET_COOKIE,
                HeaderValue::from_str(
                    format!(
                        "SEICHI_PORTAL__SESSION_ID={session_id}; Max-Age={expires}; Path=/; Secure; HttpOnly"
                    )
                    .as_str(),
                )
                .unwrap(),
            )]).into_response())
        }
        Ok(None) => Ok((
            StatusCode::UNAUTHORIZED,
            Json(json!({ "reason": "invalid token" })),
        )
            .into_response()),
        Err(err) => Err(handle_error(err)),
    }
}

pub async fn end_session(
    State(repository): State<RealInfrastructureRepository>,
    typed_header: Result<TypedHeader<Authorization<Bearer>>, TypedHeaderRejection>,
) -> Result<impl IntoResponse, Response> {
    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };

    let TypedHeader(auth) = typed_header.map_err_to_error().map_err(handle_error)?;

    let session_id = auth.token();
    user_use_case
        .end_user_session(session_id.to_string())
        .await
        .map_err(handle_error)?;
    Ok((
        StatusCode::OK,
        [(
            header::SET_COOKIE,
            HeaderValue::from_str(
                format!(
                    "SEICHI_PORTAL__SESSION_ID={session_id}; Max-Age=0; Path=/; Secure; HttpOnly"
                )
                .as_str(),
            )
            .unwrap(),
        )],
    )
        .into_response())
}

pub async fn link_discord(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    json: Result<Json<DiscordOAuthToken>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };

    let Json(discord_token) = json
        .map_err(Into::<PresentationError>::into)
        .map_err(Into::<Error>::into)
        .map_err(handle_error)?;

    user_use_case
        .link_discord_user(discord_token.token, user)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

pub async fn unlink_discord(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
) -> Result<impl IntoResponse, Response> {
    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };

    user_use_case
        .unlink_discord_user(user)
        .await
        .map_err(handle_error)?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
