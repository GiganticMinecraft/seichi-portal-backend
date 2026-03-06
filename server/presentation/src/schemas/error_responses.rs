use super::error_response::ErrorResponse;

#[derive(utoipa::IntoResponses)]
pub enum BadRequest {
    #[response(
        status = 400,
        description = "The server could not understand the request due to invalid syntax."
    )]
    BadRequest(ErrorResponse),
}

#[derive(utoipa::IntoResponses)]
pub enum Unauthorized {
    #[response(status = 401, description = "Access is unauthorized.")]
    Unauthorized(ErrorResponse),
}

#[derive(utoipa::IntoResponses)]
pub enum Forbidden {
    #[response(status = 403, description = "Access is forbidden.")]
    Forbidden(ErrorResponse),
}

#[derive(utoipa::IntoResponses)]
pub enum NotFound {
    #[response(
        status = 404,
        description = "The server cannot find the requested resource."
    )]
    NotFound(ErrorResponse),
}

#[derive(utoipa::IntoResponses)]
pub enum UnprocessableEntity {
    #[response(status = 422, description = "Client error")]
    UnprocessableEntity(ErrorResponse),
}

#[derive(utoipa::IntoResponses)]
pub enum InternalServerError {
    #[response(status = 500, description = "Server error")]
    InternalServerError(ErrorResponse),
}
