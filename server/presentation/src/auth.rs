use axum::response::IntoResponse;
use axum::{
    Json, RequestExt,
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use axum_extra::{
    extract::TypedHeader,
    headers::{Authorization, authorization::Bearer},
};
use common::config::ENV;
use domain::{
    repository::Repositories,
    user::models::{Role::Administrator, User},
};
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::user::UserUseCase;
use uuid::uuid;

pub async fn auth(
    State(repository): State<RealInfrastructureRepository>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, Response> {
    let ignore_auth_paths = ["/session"];
    if ignore_auth_paths.contains(&request.uri().path()) {
        return Ok(next.run(request).await);
    }

    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };

    let auth = request
        .extract_parts::<TypedHeader<Authorization<Bearer>>>()
        .await
        .map_err(|_| (StatusCode::UNAUTHORIZED, Json(json!({"errorCode": "UNAUTHORIZED", "reason": "Authorization header is missing."}))).into_response())?;

    let session_id = auth.token();

    let user = if ENV.name == "local" && session_id == "debug_user" {
        User {
            name: "debug_user".to_string(),
            id: uuid!("478911be-3356-46c1-936e-fb14b71bf282"),
            role: Administrator,
        }
    } else {
        match user_use_case
            .fetch_user_by_session_id(session_id.to_string())
            .await
            .map_err(|_| (StatusCode::UNAUTHORIZED, Json(json!({"errorCode": "UNAUTHORIZED", "reason": "Failed to retrieve user by session id."}))).into_response())?
        {
            Some(user) => user,
            None => return Err((StatusCode::UNAUTHORIZED, Json(json!({"errorCode": "UNAUTHORIZED", "reason": "Invalid session id."}))).into_response()),
        }
    };

    match user_use_case.upsert_user(&user, user.to_owned()).await {
        Ok(_) => {
            request.extensions_mut().insert(user);

            let response = next.run(request).await;
            Ok(response)
        }
        Err(err) => {
            tracing::error!("{}", err);
            Err((StatusCode::UNAUTHORIZED, Json(json!({"errorCode": "Internal server error", "reason": "Authentication middleware error."}))).into_response())
        }
    }
}
