use axum::{
    extract::{State, TypedHeader},
    headers::authorization::{Authorization, Bearer},
    http::{HeaderValue, Method, Request, StatusCode},
    middleware::Next,
    response::Response,
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
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use resource::repository::RealInfrastructureRepository;
use usecase::user::UserUseCase;
use uuid::uuid;

pub async fn auth<B>(
    State(repository): State<RealInfrastructureRepository>,
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    mut request: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    let token = auth.token();

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

        serde_json::from_str::<User>(
            response
                .text()
                .await
                .map_err(|_| StatusCode::UNAUTHORIZED)?
                .as_str(),
        )
        .map_err(|_| StatusCode::UNAUTHORIZED)?
    };

    let static_endpoints_allowed_for_standard_users =
        [(&Method::GET, "/forms"), (&Method::POST, "/forms/answers")];

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

    let user_use_case = UserUseCase {
        repository: repository.user_repository(),
    };

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
