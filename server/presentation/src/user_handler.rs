use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use domain::{
    repository::Repositories,
    user::models::{Role, User},
};
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::user::UserUseCase;
use uuid::Uuid;

pub async fn get_my_user_info(Extension(user): Extension<User>) -> impl IntoResponse {
    (StatusCode::OK, Json(json!(user))).into_response()
}

pub async fn patch_user_role(
    State(repository): State<RealInfrastructureRepository>,
    Path(uuid): Path<Uuid>,
    Query(role): Query<Role>,
) -> impl IntoResponse {
    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };

    match user_use_case.patch_user_role(uuid, role).await {
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
