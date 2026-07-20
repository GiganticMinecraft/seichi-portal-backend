use std::sync::Arc;

use axum::extract::rejection::QueryRejection;
use axum::response::Response;
use axum::{
    Extension, Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use domain::{
    account::models::AccountUser, auth::Actor, repository::Repositories,
    search::models::SearchableFieldsWithOperation,
};
use errors::{Error, ErrorExtra, presentation::PresentationError};
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use tokio::sync::{Notify, mpsc::Receiver};
use usecase::search::SearchUseCase;

use crate::schemas::error_responses::*;
use crate::{
    handlers::error_handler::handle_error,
    schemas::search_schemas::{
        AnswerSearchResult, CrossSearchResult, SearchQuery, UserSearchResult,
    },
};

#[derive(utoipa::IntoResponses)]
pub enum CrossSearchResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(CrossSearchResult),
}

#[derive(utoipa::IntoResponses)]
pub enum UserSearchResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(UserSearchResult),
}

#[derive(utoipa::IntoResponses)]
pub enum AnswerSearchResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(AnswerSearchResult),
}

impl IntoResponse for CrossSearchResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(json!(body))).into_response(),
        }
    }
}

impl IntoResponse for UserSearchResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(json!(body))).into_response(),
        }
    }
}

impl IntoResponse for AnswerSearchResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(json!(body))).into_response(),
        }
    }
}

fn required_query(query: Result<Query<SearchQuery>, QueryRejection>) -> Result<String, Error> {
    let Query(search_query) = query.map_err_to_error()?;
    search_query
        .query
        .map(|query| query.into_inner())
        .ok_or_else(|| {
            Error::from(PresentationError::QueryRejection {
                cause: "query is required".to_string(),
            })
        })
}

#[utoipa::path(
    get,
    path = "/search",
    summary = "横断検索を行う",
    params(
        ("query" = String, Query, description = "Search query"),
    ),
    responses(
        CrossSearchResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Search"
)]
pub async fn cross_search(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    query: Result<Query<SearchQuery>, QueryRejection>,
) -> Result<CrossSearchResponse, Response> {
    let search_use_case = SearchUseCase {
        search_repository: repository.search_repository(),
        active_form_repository: repository.active_form_repository(),
        form_answer_label_repository: repository.answer_label_repository(),
        form_label_repository: repository.form_label_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_repository: repository.comment_repository(),
    };

    let query = required_query(query).map_err(handle_error)?;

    let result = search_use_case
        .cross_search(&user, query)
        .await
        .map_err(handle_error)?;
    Ok(CrossSearchResponse::Ok(CrossSearchResult::from_output(
        &Actor::from(user),
        result,
    )))
}

#[utoipa::path(
    get,
    path = "/search/users",
    summary = "ユーザー検索を行う",
    params(
        ("query" = String, Query, description = "Search query"),
    ),
    responses(
        UserSearchResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Search"
)]
pub async fn search_users(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    query: Result<Query<SearchQuery>, QueryRejection>,
) -> Result<UserSearchResponse, Response> {
    let search_use_case = SearchUseCase {
        search_repository: repository.search_repository(),
        active_form_repository: repository.active_form_repository(),
        form_answer_label_repository: repository.answer_label_repository(),
        form_label_repository: repository.form_label_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_repository: repository.comment_repository(),
    };

    let query = required_query(query).map_err(handle_error)?;

    let users = search_use_case
        .search_users(&user, query)
        .await
        .map_err(handle_error)?;

    Ok(UserSearchResponse::Ok(UserSearchResult {
        users: users.into_iter().map(Into::into).collect(),
    }))
}

#[utoipa::path(
    get,
    path = "/search/answers",
    summary = "回答検索を行う",
    params(
        ("query" = String, Query, description = "Search query"),
    ),
    responses(
        AnswerSearchResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Search"
)]
pub async fn search_answers(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    query: Result<Query<SearchQuery>, QueryRejection>,
) -> Result<AnswerSearchResponse, Response> {
    let search_use_case = SearchUseCase {
        search_repository: repository.search_repository(),
        active_form_repository: repository.active_form_repository(),
        form_answer_label_repository: repository.answer_label_repository(),
        form_label_repository: repository.form_label_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_repository: repository.comment_repository(),
    };

    let query = required_query(query).map_err(handle_error)?;

    let answers = search_use_case
        .search_answers(&user, query)
        .await
        .map_err(handle_error)?;

    Ok(AnswerSearchResponse::Ok(answers.into()))
}

pub async fn start_sync(
    repository: RealInfrastructureRepository,
    receiver: Receiver<SearchableFieldsWithOperation>,
    shutdown_notifier: Arc<Notify>,
) -> Result<(), Error> {
    let search_use_case = SearchUseCase {
        search_repository: repository.search_repository(),
        active_form_repository: repository.active_form_repository(),
        form_answer_label_repository: repository.answer_label_repository(),
        form_label_repository: repository.form_label_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_repository: repository.comment_repository(),
    };

    search_use_case
        .start_sync(receiver, shutdown_notifier)
        .await
}

pub async fn start_watch_out_of_sync(
    repository: RealInfrastructureRepository,
    shutdown_notifier: Arc<Notify>,
) -> Result<(), Error> {
    let search_use_case = SearchUseCase {
        search_repository: repository.search_repository(),
        active_form_repository: repository.active_form_repository(),
        form_answer_label_repository: repository.answer_label_repository(),
        form_label_repository: repository.form_label_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_repository: repository.comment_repository(),
    };

    search_use_case
        .start_watch_out_of_sync(shutdown_notifier)
        .await
}

pub async fn initialize_search_engine(
    repository: RealInfrastructureRepository,
) -> Result<(), Error> {
    let search_use_case = SearchUseCase {
        search_repository: repository.search_repository(),
        active_form_repository: repository.active_form_repository(),
        form_answer_label_repository: repository.answer_label_repository(),
        form_label_repository: repository.form_label_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_repository: repository.comment_repository(),
    };

    search_use_case.initialize_search_engine().await
}
