use axum::{
    extract::TypedHeader,
    headers::authorization::{Authorization, Bearer},
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use common::config::ENV;
use domain::user::models::User;
use uuid::uuid;

pub async fn auth<B>(
    TypedHeader(auth): TypedHeader<Authorization<Bearer>>,
    mut request: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    //todo: フロントエンド側から何が返ってくるかがわかったらユーザー認証を実装する
    if ENV.name == "local" && auth.token() == "debug_user" {
        let user = User {
            name: "test_user".to_string(),
            uuid: uuid!("478911be-3356-46c1-936e-fb14b71bf282"),
        };
        request.extensions_mut().insert(user);
        let response = next.run(request).await;
        Ok(response)
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}
