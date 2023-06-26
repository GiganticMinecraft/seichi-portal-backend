use axum::{
    extract::TypedHeader,
    headers::authorization::{Authorization, Bearer},
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use common::config::ENV;

#[derive(Debug, Clone)]
pub struct User {
    pub name: String,
}

pub async fn auth<B>(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    mut request: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    if ENV.name == "local" && auth.token() == "debug_user" {
        let user = User {
            name: "test_user".to_string(),
        };
        request.extensions_mut().insert(user);
        let response = next.run(request).await;
        Ok(response)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}
