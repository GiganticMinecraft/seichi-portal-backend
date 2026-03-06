use axum::response::IntoResponse;
use axum::{
    Json, RequestExt,
    body::Body,
    extract::State,
    http::{Request, StatusCode, header},
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
    let ignore_auth_paths = ["/session", "/health"];
    let ignore_auth_path_prefixes = ["/swagger-ui", "/api-docs"];
    if ignore_auth_paths.contains(&request.uri().path())
        || ignore_auth_path_prefixes
            .iter()
            .any(|prefix| request.uri().path().starts_with(prefix))
    {
        return Ok(next.run(request).await);
    }

    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };

    let auth = request
        .extract_parts::<TypedHeader<Authorization<Bearer>>>()
        .await
        .map_err(|_| {
            (
                StatusCode::UNAUTHORIZED,
                [(header::CONTENT_TYPE, "application/problem+json")],
                Json(json!({
                    "type": "about:blank",
                    "title": "Unauthorized",
                    "status": 401,
                    "detail": "Authorization header is missing.",
                    "errorCode": "UNAUTHORIZED"
                })),
            )
                .into_response()
        })?;

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
            .map_err(|_| {
                (
                    StatusCode::UNAUTHORIZED,
                    [(header::CONTENT_TYPE, "application/problem+json")],
                    Json(json!({
                        "type": "about:blank",
                        "title": "Unauthorized",
                        "status": 401,
                        "detail": "Failed to retrieve user by session id.",
                        "errorCode": "UNAUTHORIZED"
                    })),
                )
                    .into_response()
            })? {
            Some(user) => user,
            None => {
                return Err((
                    StatusCode::UNAUTHORIZED,
                    [(header::CONTENT_TYPE, "application/problem+json")],
                    Json(json!({
                        "type": "about:blank",
                        "title": "Unauthorized",
                        "status": 401,
                        "detail": "Invalid session id.",
                        "errorCode": "UNAUTHORIZED"
                    })),
                )
                    .into_response());
            }
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
            Err((
                StatusCode::UNAUTHORIZED,
                [(header::CONTENT_TYPE, "application/problem+json")],
                Json(json!({
                    "type": "about:blank",
                    "title": "Unauthorized",
                    "status": 401,
                    "detail": "Authentication middleware error.",
                    "errorCode": "UNAUTHORIZED"
                })),
            )
                .into_response())
        }
    }
}
