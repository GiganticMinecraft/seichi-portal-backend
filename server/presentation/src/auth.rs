use axum::{
    extract::TypedHeader,
    headers::authorization::{Authorization, Bearer},
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};

pub async fn auth<B>(
    TypedHeader(_auth): TypedHeader<Authorization<Bearer>>,
    _request: Request<B>,
    _next: Next<B>,
) -> Result<Response, StatusCode> {
    Err(StatusCode::UNAUTHORIZED)
}
