use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use axum_extra::{
    extract::TypedHeader,
    headers::{authorization::Bearer, Authorization},
};
use common::config::ENV;
use domain::{
    repository::Repositories,
    user::models::{Role::Administrator, User},
};
use resource::repository::RealInfrastructureRepository;
use usecase::user::UserUseCase;
use uuid::uuid;

pub async fn auth(
    State(repository): State<RealInfrastructureRepository>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let ignore_auth_paths = ["/session"];
    if ignore_auth_paths.contains(&request.uri().path()) {
        return Ok(next.run(request).await);
    }

    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };

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
            .map_err(|_| StatusCode::UNAUTHORIZED)?
        {
            Some(user) => user,
            None => return Err(StatusCode::UNAUTHORIZED),
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
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
