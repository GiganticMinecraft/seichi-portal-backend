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
pub enum PayloadTooLarge {
    #[response(
        status = 413,
        description = "The request payload is too large.",
        content_type = "application/problem+json"
    )]
    PayloadTooLarge(ErrorResponse),
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

#[derive(utoipa::IntoResponses)]
pub enum ServiceUnavailable {
    #[response(
        status = 503,
        description = "The server is temporarily unable to handle the request.",
        content_type = "application/problem+json"
    )]
    ServiceUnavailable(ErrorResponse),
}

#[derive(utoipa::IntoResponses)]
pub enum SessionCreateUnauthorized {
    #[response(
        status = 401,
        description = "Access is unauthorized.",
        content_type = "application/problem+json",
        examples(
            ("invalid_authorization_header" = (
                summary = "Authorization header is invalid",
                value = json!({
                    "type": "about:blank",
                    "title": "Unauthorized",
                    "status": 401,
                    "detail": "Invalid authorization header.",
                    "errorCode": "UNAUTHORIZED"
                })
            )),
            ("invalid_minecraft_token" = (
                summary = "Minecraft access token is invalid",
                value = json!({
                    "type": "about:blank",
                    "title": "Unauthorized",
                    "status": 401,
                    "detail": "Minecraft access token is invalid.",
                    "errorCode": "MINECRAFT_TOKEN_INVALID"
                })
            ))
        )
    )]
    Unauthorized(ErrorResponse),
}

#[derive(utoipa::IntoResponses)]
pub enum SessionCreateProfileNotFound {
    #[response(
        status = 404,
        description = "The Minecraft profile endpoint reported that no profile was found.",
        content_type = "application/problem+json",
        example = json!({
            "type": "about:blank",
            "title": "Not Found",
            "status": 404,
            "detail": "Minecraft profile was not found.",
            "errorCode": "MINECRAFT_PROFILE_NOT_FOUND"
        })
    )]
    ProfileNotFound(ErrorResponse),
}

#[derive(utoipa::IntoResponses)]
pub enum SessionCreateBadGateway {
    #[response(
        status = 502,
        description = "The Minecraft profile service returned an invalid or unexpected response.",
        content_type = "application/problem+json",
        examples(
            ("upstream_http_error" = (
                summary = "Minecraft profile service returned an HTTP error",
                value = json!({
                    "type": "about:blank",
                    "title": "Bad Gateway",
                    "status": 502,
                    "detail": "Minecraft profile service returned an unexpected HTTP response.",
                    "errorCode": "MINECRAFT_PROFILE_UPSTREAM_ERROR"
                })
            )),
            ("invalid_upstream_response" = (
                summary = "Minecraft profile service returned an invalid response",
                value = json!({
                    "type": "about:blank",
                    "title": "Bad Gateway",
                    "status": 502,
                    "detail": "Minecraft profile service returned an invalid response.",
                    "errorCode": "MINECRAFT_PROFILE_INVALID_RESPONSE"
                })
            ))
        )
    )]
    BadGateway(ErrorResponse),
}

#[derive(utoipa::IntoResponses)]
pub enum SessionCreateServiceUnavailable {
    #[response(
        status = 503,
        description = "The Minecraft profile service could not be reached or is temporarily unavailable.",
        content_type = "application/problem+json",
        examples(
            ("upstream_service_unavailable" = (
                summary = "Minecraft profile service returned a server error",
                value = json!({
                    "type": "about:blank",
                    "title": "Service Unavailable",
                    "status": 503,
                    "detail": "Minecraft profile service is temporarily unavailable.",
                    "errorCode": "MINECRAFT_PROFILE_SERVICE_UNAVAILABLE"
                })
            )),
            ("profile_request_failed" = (
                summary = "Minecraft profile service could not be reached",
                value = json!({
                    "type": "about:blank",
                    "title": "Service Unavailable",
                    "status": 503,
                    "detail": "Could not communicate with Minecraft profile service.",
                    "errorCode": "MINECRAFT_PROFILE_REQUEST_FAILED"
                })
            ))
        )
    )]
    ServiceUnavailable(ErrorResponse),
}
