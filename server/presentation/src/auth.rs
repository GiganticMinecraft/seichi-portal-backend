use axum::http::HeaderValue;
use axum::{
    extract::TypedHeader,
    headers::authorization::{Authorization, Bearer},
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use common::config::ENV;
use domain::user::models::User;
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use uuid::uuid;

pub async fn auth<B>(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    mut request: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    let token = auth.token();

    if ENV.name == "local" && token == "debug_user" {
        let user = User {
            name: "test_user".to_string(),
            id: uuid!("478911be-3356-46c1-936e-fb14b71bf282"),
        };
        request.extensions_mut().insert(user);
        let response = next.run(request).await;
        Ok(response)
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

        println!(
            "{:?}",
            serde_json::from_str::<User>(
                response
                    .text()
                    .await
                    .map_err(|_| StatusCode::UNAUTHORIZED)?
                    .as_str()
            )
        );

        Err(StatusCode::UNAUTHORIZED)
    }
}
