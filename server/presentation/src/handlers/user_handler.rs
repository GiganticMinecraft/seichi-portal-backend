use axum::{
    Extension, Json,
    extract::rejection::{JsonRejection, PathRejection, QueryRejection},
    extract::{Path, Query, State},
    http::{HeaderValue, StatusCode, header},
    response::IntoResponse,
};
use axum_extra::{
    TypedHeader,
    headers::{Authorization, authorization::Bearer},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use domain::{
    account::models::{AccountUser, UserPagePosition, UserSessionExpires},
    form::answer::AnswerSubmitterRestrictionReason,
    pagination::{PageLimit, PageRequest},
    repository::Repositories,
};
use resource::repository::RealInfrastructureRepository;
use serde::{Deserialize, Serialize};
use serde_json::json;
use usecase::{answer_submitter_restriction::AnswerSubmitterRestrictionUseCase, user::UserUseCase};
use uuid::Uuid;

use crate::schemas::error_responses::*;
use crate::schemas::user::{
    AnswerSubmitterRestrictionHistoryResponse, AnswerSubmitterRestrictionRequest,
    AnswerSubmitterRestrictionResponse, UserGroupRequest, UserGroupSchema, UserInfoResponse,
    UserListPageResponse, UserListQuery, UserSchema, UserUpdateSchema,
};
use crate::{handlers::error_handler::handle_error, schemas::user::DiscordOAuthToken};
use axum::response::Response;
use axum_extra::typed_header::TypedHeaderRejection;
use errors::presentation::PresentationError;
use errors::{Error, ErrorExtra};

#[derive(utoipa::IntoResponses)]
pub enum GetUserInfoResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(UserInfoResponse),
}

impl IntoResponse for GetUserInfoResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum PatchUserRoleResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(UserSchema),
}

impl IntoResponse for PatchUserRoleResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum UserListResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(UserListPageResponse),
}

#[derive(utoipa::IntoResponses)]
pub enum UserGroupListResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(Vec<UserGroupSchema>),
}

impl IntoResponse for UserGroupListResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum UserGroupUserListResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(Vec<UserSchema>),
}

impl IntoResponse for UserGroupUserListResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum UserGroupResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(UserGroupSchema),
    #[response(status = 201, description = "The resource has been created.")]
    Created(UserGroupSchema),
}

impl IntoResponse for UserGroupResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
            Self::Created(body) => (StatusCode::CREATED, Json(body)).into_response(),
        }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum UserGroupMembershipResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(UserSchema),
}

impl IntoResponse for UserGroupMembershipResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum DeleteUserGroupResponse {
    #[response(status = 204, description = "The resource has been deleted.")]
    NoContent,
}

impl IntoResponse for DeleteUserGroupResponse {
    fn into_response(self) -> Response {
        match self {
            Self::NoContent => StatusCode::NO_CONTENT.into_response(),
        }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum GetAnswerSubmitterRestrictionResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(Option<AnswerSubmitterRestrictionResponse>),
}

impl IntoResponse for GetAnswerSubmitterRestrictionResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum GetAnswerSubmitterRestrictionHistoryResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(Vec<AnswerSubmitterRestrictionHistoryResponse>),
}

impl IntoResponse for GetAnswerSubmitterRestrictionHistoryResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum PutAnswerSubmitterRestrictionResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(AnswerSubmitterRestrictionResponse),
}

impl IntoResponse for PutAnswerSubmitterRestrictionResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

impl IntoResponse for UserListResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

#[derive(Deserialize, Serialize)]
struct UserListCursor {
    after_user_id: Uuid,
}

fn bad_query(message: impl Into<String>) -> Error {
    Error::from(PresentationError::QueryRejection {
        cause: message.into(),
    })
}

fn decode_user_list_cursor(cursor: &str) -> Result<UserPagePosition, Error> {
    let decoded = URL_SAFE_NO_PAD
        .decode(cursor)
        .map_err(|_| bad_query("Invalid cursor."))?;
    let cursor = serde_json::from_slice::<UserListCursor>(&decoded)
        .map_err(|_| bad_query("Invalid cursor."))?;

    Ok(UserPagePosition::new(cursor.after_user_id.into()))
}

fn encode_user_list_cursor(position: UserPagePosition) -> Result<String, Error> {
    let cursor = UserListCursor {
        after_user_id: position.last_user_id().into_inner(),
    };
    let bytes = serde_json::to_vec(&cursor).map_err(|_| bad_query("Invalid cursor."))?;

    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

fn user_list_page_request(query: UserListQuery) -> Result<PageRequest<UserPagePosition>, Error> {
    let limit = match query.limit {
        Some(limit) => PageLimit::try_new(limit)
            .map_err(|err| bad_query(format!("Invalid limit: {}.", err.value())))?,
        None => PageLimit::default_limit(),
    };
    let after = query
        .cursor
        .as_deref()
        .map(decode_user_list_cursor)
        .transpose()?;

    Ok(PageRequest::new(after, limit))
}

#[utoipa::path(
    get,
    path = "/users/me",
    summary = "自分のユーザー情報の取得",
    responses(
        GetUserInfoResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Users"
)]
pub async fn get_my_user_info(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
) -> Result<GetUserInfoResponse, Response> {
    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };

    let user_profile = user_use_case
        .fetch_user_information(&user, user.id().into_inner())
        .await
        .map_err(handle_error)?;
    let discord_user_id_with_name = user_profile.discord_user.map(|user| {
        (
            user.id().to_owned().into_inner(),
            user.name().to_owned().into_inner(),
        )
    });
    Ok(GetUserInfoResponse::Ok(UserInfoResponse {
        id: user_profile.user.id().to_string(),
        name: user_profile.user.name().to_owned(),
        role: user_profile.user.role().to_string(),
        groups: user_profile
            .user
            .groups()
            .iter()
            .cloned()
            .map(Into::into)
            .collect(),
        discord_user_id: discord_user_id_with_name.to_owned().map(|(id, _)| id),
        discord_username: discord_user_id_with_name.map(|(_, name)| name),
    }))
}

#[utoipa::path(
    get,
    path = "/users/{uuid}",
    summary = "ユーザーの取得",
    params(
        ("uuid" = String, Path, description = "User UUID"),
    ),
    responses(
        GetUserInfoResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Users"
)]
pub async fn get_user_info(
    Extension(actor): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Result<GetUserInfoResponse, Response> {
    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };

    let Path(uuid) = path.map_err_to_error().map_err(handle_error)?;

    let user_profile = user_use_case
        .fetch_user_information(&actor, uuid)
        .await
        .map_err(handle_error)?;
    let discord_user_id_with_name = user_profile.discord_user.map(|user| {
        (
            user.id().to_owned().into_inner(),
            user.name().to_owned().into_inner(),
        )
    });
    Ok(GetUserInfoResponse::Ok(UserInfoResponse {
        id: user_profile.user.id().to_string(),
        name: user_profile.user.name().to_owned(),
        role: user_profile.user.role().to_string(),
        groups: user_profile
            .user
            .groups()
            .iter()
            .cloned()
            .map(Into::into)
            .collect(),
        discord_user_id: discord_user_id_with_name.to_owned().map(|(id, _)| id),
        discord_username: discord_user_id_with_name.map(|(_, name)| name),
    }))
}

#[utoipa::path(
    patch,
    path = "/users/{uuid}",
    summary = "ユーザーの更新",
    params(
        ("uuid" = String, Path, description = "User UUID"),
    ),
    request_body = UserUpdateSchema,
    responses(
        PatchUserRoleResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
    ),
    security(("bearer" = [])),
    tag = "Users"
)]
pub async fn patch_user_role(
    Extension(actor): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<Uuid>, PathRejection>,
    json: Result<Json<UserUpdateSchema>, JsonRejection>,
) -> Result<PatchUserRoleResponse, Response> {
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

    Ok(PatchUserRoleResponse::Ok(user.into()))
}

#[utoipa::path(
    get,
    path = "/users",
    summary = "ユーザーの一覧取得",
    params(
        UserListQuery,
    ),
    responses(
        UserListResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Users"
)]
pub async fn user_list(
    Extension(actor): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    query: Result<Query<UserListQuery>, QueryRejection>,
) -> Result<UserListResponse, Response> {
    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };
    let Query(query) = query.map_err_to_error().map_err(handle_error)?;
    let request = user_list_page_request(query).map_err(handle_error)?;

    let page = user_use_case
        .fetch_users_page(&actor, request)
        .await
        .map_err(handle_error)?;
    let (users, next) = page.into_parts();
    let next_cursor = next
        .map(encode_user_list_cursor)
        .transpose()
        .map_err(handle_error)?;

    Ok(UserListResponse::Ok(UserListPageResponse {
        items: users.into_iter().map(Into::into).collect(),
        next_cursor,
    }))
}

#[utoipa::path(
    post,
    path = "/user-groups",
    summary = "ユーザーグループの作成",
    request_body = UserGroupRequest,
    responses(
        UserGroupResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "User Groups"
)]
pub async fn create_user_group(
    Extension(actor): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    json: Result<Json<UserGroupRequest>, JsonRejection>,
) -> Result<UserGroupResponse, Response> {
    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };
    let Json(request) = json.map_err_to_error().map_err(handle_error)?;

    let group = user_use_case
        .create_user_group(&actor, request.name)
        .await
        .map_err(handle_error)?;

    Ok(UserGroupResponse::Created(group.into()))
}

#[utoipa::path(
    get,
    path = "/user-groups",
    summary = "ユーザーグループの一覧取得",
    responses(
        UserGroupListResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "User Groups"
)]
pub async fn user_group_list(
    Extension(actor): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
) -> Result<UserGroupListResponse, Response> {
    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };

    let groups = user_use_case
        .fetch_user_groups(&actor)
        .await
        .map_err(handle_error)?;

    Ok(UserGroupListResponse::Ok(
        groups.into_iter().map(Into::into).collect(),
    ))
}

#[utoipa::path(
    get,
    path = "/user-groups/{group_id}/users",
    summary = "ユーザーグループに所属するユーザーの一覧取得",
    params(
        ("group_id" = String, Path, description = "User group UUID"),
    ),
    responses(
        UserGroupUserListResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "User Groups"
)]
pub async fn user_group_user_list(
    Extension(actor): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Result<UserGroupUserListResponse, Response> {
    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };
    let Path(group_id) = path.map_err_to_error().map_err(handle_error)?;

    let users = user_use_case
        .fetch_users_by_group(&actor, group_id.into())
        .await
        .map_err(handle_error)?;

    Ok(UserGroupUserListResponse::Ok(
        users.into_iter().map(Into::into).collect(),
    ))
}

#[utoipa::path(
    patch,
    path = "/user-groups/{group_id}",
    summary = "ユーザーグループの更新",
    params(
        ("group_id" = String, Path, description = "User group UUID"),
    ),
    request_body = UserGroupRequest,
    responses(
        UserGroupResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "User Groups"
)]
pub async fn update_user_group(
    Extension(actor): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<Uuid>, PathRejection>,
    json: Result<Json<UserGroupRequest>, JsonRejection>,
) -> Result<UserGroupResponse, Response> {
    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };
    let Path(group_id) = path.map_err_to_error().map_err(handle_error)?;
    let Json(request) = json.map_err_to_error().map_err(handle_error)?;

    let group = user_use_case
        .update_user_group(&actor, group_id.into(), request.name)
        .await
        .map_err(handle_error)?;

    Ok(UserGroupResponse::Ok(group.into()))
}

#[utoipa::path(
    delete,
    path = "/user-groups/{group_id}",
    summary = "ユーザーグループの削除",
    params(
        ("group_id" = String, Path, description = "User group UUID"),
    ),
    responses(
        DeleteUserGroupResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "User Groups"
)]
pub async fn delete_user_group(
    Extension(actor): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Result<DeleteUserGroupResponse, Response> {
    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };
    let Path(group_id) = path.map_err_to_error().map_err(handle_error)?;

    user_use_case
        .delete_user_group(&actor, group_id.into())
        .await
        .map_err(handle_error)?;

    Ok(DeleteUserGroupResponse::NoContent)
}

#[utoipa::path(
    put,
    path = "/user-groups/{group_id}/users/{user_id}",
    summary = "ユーザーをグループに追加",
    params(
        ("group_id" = String, Path, description = "User group UUID"),
        ("user_id" = String, Path, description = "User UUID"),
    ),
    responses(
        UserGroupMembershipResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "User Groups"
)]
pub async fn add_user_to_group(
    Extension(actor): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(Uuid, Uuid)>, PathRejection>,
) -> Result<UserGroupMembershipResponse, Response> {
    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };
    let Path((group_id, user_id)) = path.map_err_to_error().map_err(handle_error)?;

    let user = user_use_case
        .add_user_to_group(&actor, group_id.into(), user_id)
        .await
        .map_err(handle_error)?;

    Ok(UserGroupMembershipResponse::Ok(user.into()))
}

#[utoipa::path(
    delete,
    path = "/user-groups/{group_id}/users/{user_id}",
    summary = "ユーザーをグループから削除",
    params(
        ("group_id" = String, Path, description = "User group UUID"),
        ("user_id" = String, Path, description = "User UUID"),
    ),
    responses(
        UserGroupMembershipResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "User Groups"
)]
pub async fn remove_user_from_group(
    Extension(actor): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(Uuid, Uuid)>, PathRejection>,
) -> Result<UserGroupMembershipResponse, Response> {
    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };
    let Path((group_id, user_id)) = path.map_err_to_error().map_err(handle_error)?;

    let user = user_use_case
        .remove_user_from_group(&actor, group_id.into(), user_id)
        .await
        .map_err(handle_error)?;

    Ok(UserGroupMembershipResponse::Ok(user.into()))
}

#[utoipa::path(
    get,
    path = "/users/{uuid}/answer-submitter-restriction",
    summary = "回答投稿者の有効な回答投稿制限の取得",
    params(
        ("uuid" = String, Path, description = "User UUID"),
    ),
    responses(
        GetAnswerSubmitterRestrictionResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Users"
)]
pub async fn get_answer_submitter_restriction(
    Extension(actor): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Result<GetAnswerSubmitterRestrictionResponse, Response> {
    let restriction_use_case = AnswerSubmitterRestrictionUseCase {
        user_repository: repository.user_repository(),
        restriction_repository: repository.answer_submitter_restriction_repository(),
    };

    let Path(uuid) = path.map_err_to_error().map_err(handle_error)?;
    let restriction = restriction_use_case
        .fetch_active(&actor, uuid)
        .await
        .map_err(handle_error)?;

    Ok(GetAnswerSubmitterRestrictionResponse::Ok(
        restriction.map(Into::into),
    ))
}

#[utoipa::path(
    get,
    path = "/users/{uuid}/answer-submitter-restriction/history",
    summary = "回答投稿者の回答投稿制限履歴の取得",
    params(
        ("uuid" = String, Path, description = "User UUID"),
    ),
    responses(
        GetAnswerSubmitterRestrictionHistoryResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Users"
)]
pub async fn get_answer_submitter_restriction_history(
    Extension(actor): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Result<GetAnswerSubmitterRestrictionHistoryResponse, Response> {
    let restriction_use_case = AnswerSubmitterRestrictionUseCase {
        user_repository: repository.user_repository(),
        restriction_repository: repository.answer_submitter_restriction_repository(),
    };

    let Path(uuid) = path.map_err_to_error().map_err(handle_error)?;
    let restrictions = restriction_use_case
        .list_history(&actor, uuid)
        .await
        .map_err(handle_error)?;

    Ok(GetAnswerSubmitterRestrictionHistoryResponse::Ok(
        restrictions.into_iter().map(Into::into).collect(),
    ))
}

#[utoipa::path(
    put,
    path = "/users/{uuid}/answer-submitter-restriction",
    summary = "回答投稿者の回答投稿を制限する",
    params(
        ("uuid" = String, Path, description = "User UUID"),
    ),
    request_body = AnswerSubmitterRestrictionRequest,
    responses(
        PutAnswerSubmitterRestrictionResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Users"
)]
pub async fn put_answer_submitter_restriction(
    Extension(actor): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<Uuid>, PathRejection>,
    json: Result<Json<AnswerSubmitterRestrictionRequest>, JsonRejection>,
) -> Result<PutAnswerSubmitterRestrictionResponse, Response> {
    let restriction_use_case = AnswerSubmitterRestrictionUseCase {
        user_repository: repository.user_repository(),
        restriction_repository: repository.answer_submitter_restriction_repository(),
    };

    let Path(uuid) = path.map_err_to_error().map_err(handle_error)?;
    let Json(request) = json.map_err_to_error().map_err(handle_error)?;
    let restriction = restriction_use_case
        .restrict(
            &actor,
            uuid,
            AnswerSubmitterRestrictionReason::new(request.reason),
            request.expires_at,
        )
        .await
        .map_err(handle_error)?;

    Ok(PutAnswerSubmitterRestrictionResponse::Ok(
        restriction.into(),
    ))
}

#[utoipa::path(
    delete,
    path = "/users/{uuid}/answer-submitter-restriction",
    summary = "回答投稿者の回答投稿制限を解除する",
    params(
        ("uuid" = String, Path, description = "User UUID"),
    ),
    responses(
        (status = 204, description = "There is no content to send for this request, but the headers may be useful."),
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Users"
)]
pub async fn delete_answer_submitter_restriction(
    Extension(actor): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Result<impl IntoResponse, Response> {
    let restriction_use_case = AnswerSubmitterRestrictionUseCase {
        user_repository: repository.user_repository(),
        restriction_repository: repository.answer_submitter_restriction_repository(),
    };

    let Path(uuid) = path.map_err_to_error().map_err(handle_error)?;
    restriction_use_case
        .lift(&actor, uuid)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

#[utoipa::path(
    post,
    path = "/session",
    summary = "セッションを作成する",
    request_body = super::super::schemas::session::SessionCreateSchema,
    responses(
        (status = 201, description = "The request has succeeded and a new resource has been created as a result."),
        BadRequest,
        Unauthorized,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Session"
)]
pub async fn start_session(
    State(repository): State<RealInfrastructureRepository>,
    header: Result<TypedHeader<Authorization<Bearer>>, TypedHeaderRejection>,
    json: Result<Json<UserSessionExpires>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };

    let Json(expires) = json.map_err_to_error().map_err(handle_error)?;

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
            Ok((StatusCode::CREATED, [(
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

#[utoipa::path(
    delete,
    path = "/session",
    summary = "セッションを削除する",
    responses(
        (status = 204, description = "There is no content to send for this request, but the headers may be useful."),
        BadRequest,
        Unauthorized,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Session"
)]
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
        StatusCode::NO_CONTENT,
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

#[utoipa::path(
    post,
    path = "/link-discord",
    summary = "Discord アカウントとリンクする",
    request_body = DiscordOAuthToken,
    responses(
        (status = 204, description = "There is no content to send for this request, but the headers may be useful."),
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Users"
)]
pub async fn link_discord(
    Extension(user): Extension<AccountUser>,
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

#[utoipa::path(
    delete,
    path = "/link-discord",
    summary = "Discord アカウントとのリンクを解除する",
    responses(
        (status = 204, description = "There is no content to send for this request, but the headers may be useful."),
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Users"
)]
pub async fn unlink_discord(
    Extension(user): Extension<AccountUser>,
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
