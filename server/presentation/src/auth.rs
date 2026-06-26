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
use domain::repository::Repositories;
use domain::{account::models::AccountUser, auth::Actor};
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::user::UserUseCase;

fn unauthorized_response(detail: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::CONTENT_TYPE, "application/problem+json")],
        Json(json!({
            "type": "about:blank",
            "title": "Unauthorized",
            "status": 401,
            "detail": detail,
            "errorCode": "UNAUTHORIZED"
        })),
    )
        .into_response()
}

async fn resolve_user(
    repository: &RealInfrastructureRepository,
    session_id: &str,
) -> Result<AccountUser, Response> {
    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };

    let session_user = user_use_case
        .fetch_user_by_session_id(session_id.to_string())
        .await
        .map_err(|_| unauthorized_response("Failed to retrieve user by session id."))?
        .ok_or_else(|| unauthorized_response("Invalid session id."))?;

    user_use_case
        .find_by(&session_user, session_user.id().into_inner())
        .await
        .map_err(|_| unauthorized_response("Failed to retrieve user from database."))
}

pub async fn auth(
    State(repository): State<RealInfrastructureRepository>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, Response> {
    let auth = request
        .extract_parts::<TypedHeader<Authorization<Bearer>>>()
        .await
        .map_err(|_| unauthorized_response("Authorization header is missing."))?;

    let user = resolve_user(&repository, auth.token()).await?;

    request.extensions_mut().insert(user);

    Ok(next.run(request).await)
}

pub async fn optional_auth(
    State(repository): State<RealInfrastructureRepository>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, Response> {
    let auth = request
        .extract_parts::<TypedHeader<Authorization<Bearer>>>()
        .await;

    match auth {
        Ok(auth) => {
            let user = resolve_user(&repository, auth.token()).await?;
            request.extensions_mut().insert(Actor::AccountUser(user));
        }
        Err(_) => {
            request.extensions_mut().insert(Actor::Anonymous);
        }
    }

    Ok(next.run(request).await)
}
