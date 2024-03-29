use axum::{
    body::Body,
    extract::State,
    http::{Method, Request, StatusCode},
    middleware::Next,
    response::Response,
};
use axum_extra::{
    extract::TypedHeader,
    headers::{authorization::Bearer, Authorization},
};
use common::config::ENV;
use domain::{
    repository::{user_repository::UserRepository, Repositories},
    user::models::{
        Role::{Administrator, StandardUser},
        User,
    },
};
use regex::Regex;
use reqwest::header::{HeaderValue, ACCEPT, CONTENT_TYPE};
use resource::repository::RealInfrastructureRepository;
use usecase::user::UserUseCase;
use uuid::uuid;

pub async fn auth(
    State(repository): State<RealInfrastructureRepository>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    mut request: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let token = auth.token();

    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };

    let user = if ENV.name == "local" && token == "debug_user" {
        User {
            name: "test_user".to_string(),
            id: uuid!("478911be-3356-46c1-936e-fb14b71bf282"),
            role: Administrator,
        }
    } else {
        let client = reqwest::Client::new();

        let response = client
            .get("https://api.minecraftservices.com/minecraft/profile")
            .bearer_auth(token)
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .header(ACCEPT, HeaderValue::from_static("application/json"))
            .send()
            .await
            .map_err(|_| StatusCode::UNAUTHORIZED)?;

        let parsed_user = serde_json::from_str::<User>(
            response
                .text()
                .await
                .map_err(|_| StatusCode::UNAUTHORIZED)?
                .as_str(),
        )
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

        user_use_case
            .repository
            .find_by(parsed_user.id)
            .await
            .map_err(|_| StatusCode::UNAUTHORIZED)?
            .map_or(parsed_user, |user| user)
    };

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

    match user_use_case.repository.upsert_user(&user).await {
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
