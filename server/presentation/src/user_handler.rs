use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use domain::{
    repository::Repositories,
    user::models::{RoleQuery, User},
};
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::user::UserUseCase;
use uuid::Uuid;

pub async fn get_my_user_info(Extension(user): Extension<User>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "uuid": user.id.to_string(),
            "name": user.name,
            "role": user.role.to_string()
        })),
    )
        .into_response()
}

pub async fn patch_user_role(
    State(repository): State<RealInfrastructureRepository>,
    Path(uuid): Path<Uuid>,
    Query(role): Query<RoleQuery>,
) -> impl IntoResponse {
    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };

    match user_use_case.patch_user_role(uuid, role.role).await {
        Ok(_) => (StatusCode::OK).into_response(),
        Err(err) => {
            tracing::error!("{}", err);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "reason": "unknown error" })),
            )
                .into_response()
        }
    }
}
