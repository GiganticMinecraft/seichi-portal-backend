use super::error_response::ErrorResponse;

#[derive(utoipa::IntoResponses)]
pub enum BadRequest {
    #[response(
        status = 400,
        description = "The server could not understand the request due to invalid syntax.",
        content_type = "application/problem+json"
    )]
    BadRequest(ErrorResponse),
}

#[derive(utoipa::IntoResponses)]
pub enum Unauthorized {
    #[response(
        status = 401,
        description = "Access is unauthorized.",
        content_type = "application/problem+json"
    )]
    Unauthorized(ErrorResponse),
}

#[derive(utoipa::IntoResponses)]
pub enum Forbidden {
    #[response(
        status = 403,
        description = "Access is forbidden.",
        content_type = "application/problem+json"
    )]
    Forbidden(ErrorResponse),
}

#[derive(utoipa::IntoResponses)]
pub enum NotFound {
    #[response(
        status = 404,
        description = "The server cannot find the requested resource.",
        content_type = "application/problem+json"
    )]
    NotFound(ErrorResponse),
}

#[derive(utoipa::IntoResponses)]
pub enum UnprocessableEntity {
    #[response(
        status = 422,
        description = "Client error",
        content_type = "application/problem+json"
    )]
    UnprocessableEntity(ErrorResponse),
}

#[derive(utoipa::IntoResponses)]
pub enum InternalServerError {
    #[response(
        status = 500,
        description = "Server error",
        content_type = "application/problem+json"
    )]
    InternalServerError(ErrorResponse),
}
