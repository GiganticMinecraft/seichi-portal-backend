use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderValue, Method, Request, StatusCode},
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
    user::models::{
        Role::{Administrator, StandardUser},
        User,
    },
};
use regex::Regex;
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

    let (user, session_id) = if ENV.name == "local" && session_id == "debug_user" {
        (
            User {
                name: "debug_user".to_string(),
                id: uuid!("478911be-3356-46c1-936e-fb14b71bf282"),
                role: Administrator,
            },
            "debug_user".to_string(),
        )
    } else {
        match user_use_case
            .fetch_user_by_session_id(session_id.to_string())
            .await
            .map_err(|_| StatusCode::UNAUTHORIZED)?
        {
            Some(user) => (user, session_id.to_string()),
            None => return Err(StatusCode::UNAUTHORIZED),
        }
    };

    user_use_case
        .update_user_session(session_id.to_string())
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let static_endpoints_allowed_for_standard_users = [
        (&Method::GET, "/forms"),
        (&Method::POST, "/forms/answers"),
        (&Method::POST, "/forms/answers/comment"),
        (&Method::GET, "/users"),
    ];

    // NOTE: 動的パスを指定する場合は、正規表現を埋め込む
    let dynamic_endpoints_allowed_for_standard_users = [(&Method::GET, "/forms/[^/]+/questions")];

    let is_not_allow_dynamic_endpoint = !dynamic_endpoints_allowed_for_standard_users
        .into_iter()
        .any(|(method, endpoint)| {
            let regex = Regex::new(endpoint).unwrap();

            method == request.method() && regex.is_match(request.uri().path())
        });

    if user.role == StandardUser
        && !static_endpoints_allowed_for_standard_users
            .contains(&(request.method(), request.uri().path()))
        && is_not_allow_dynamic_endpoint
    {
        // NOTE: standard_user_endpointsに存在しないMethodとエンドポイントに
        //          一般ユーザーがアクセスした場合は、アクセス権限なしとしてすべてFORBIDDENを返す。
        return Err(StatusCode::FORBIDDEN);
    }

    match user_use_case.upsert_user(&user).await {
        Ok(_) => {
            request.extensions_mut().insert(user);

            let mut response = next.run(request).await;
            let half_an_hour = 1800;

            response.headers_mut().insert(
                header::SET_COOKIE,
                HeaderValue::from_str(&format!(
                    "SEICHI_PORTAL__SESSION_ID={session_id}; Max-Age={half_an_hour}; Path=/; \
                     Secure; HttpOnly"
                ))
                .unwrap(),
            );
            Ok(response)
        }
        Err(err) => {
            tracing::error!("{}", err);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
