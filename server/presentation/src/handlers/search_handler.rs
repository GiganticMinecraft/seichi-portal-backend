use std::time::Duration;

use axum::extract::rejection::QueryRejection;
use axum::response::Response;
use axum::{
    Extension, Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use domain::{
    account::models::AccountUser, repository::Repositories,
    search::models::SearchableFieldsWithOperation,
};
use errors::{Error, ErrorExtra, presentation::PresentationError};
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use tokio::sync::{mpsc::Receiver, watch};
use tracing::{error, info};
use usecase::search::SearchUseCase;

use crate::schemas::error_responses::*;
use crate::{
    handlers::error_handler::{ApiError, handle_error},
    schemas::search_schemas::{
        AnswerSearchQuery, AnswerSearchResult, CrossSearchResult, SearchQuery, UserSearchResult,
    },
};

const SEARCH_ENGINE_INITIALIZATION_RETRY_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchEngineInitializationStatus {
    Pending,
    Ready,
    Shutdown,
}

pub fn update_search_engine_initialization_status(
    sender: &watch::Sender<SearchEngineInitializationStatus>,
    next: SearchEngineInitializationStatus,
) -> bool {
    sender.send_if_modified(|current| {
        if *current == SearchEngineInitializationStatus::Shutdown
            && next != SearchEngineInitializationStatus::Shutdown
        {
            return false;
        }
        if *current == next {
            return false;
        }

        *current = next;
        true
    })
}

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
        ServiceUnavailable,
    ),
    security(("bearer" = [])),
    tag = "Search"
)]
pub async fn cross_search(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    query: Result<Query<SearchQuery>, QueryRejection>,
) -> Result<CrossSearchResponse, ApiError> {
    let search_use_case = SearchUseCase {
        search_repository: repository.search_repository(),
        active_form_repository: repository.active_form_repository(),
        form_answer_label_repository: repository.answer_label_repository(),
        form_label_repository: repository.form_label_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_thread_repository: repository.comment_thread_repository(),
    };

    let query = required_query(query).map_err(handle_error)?;

    let result = search_use_case
        .cross_search(&user, query)
        .await
        .map_err(handle_error)?;
    Ok(CrossSearchResponse::Ok(CrossSearchResult::from_output(
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
        ServiceUnavailable,
    ),
    security(("bearer" = [])),
    tag = "Search"
)]
pub async fn search_users(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    query: Result<Query<SearchQuery>, QueryRejection>,
) -> Result<UserSearchResponse, ApiError> {
    let search_use_case = SearchUseCase {
        search_repository: repository.search_repository(),
        active_form_repository: repository.active_form_repository(),
        form_answer_label_repository: repository.answer_label_repository(),
        form_label_repository: repository.form_label_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_thread_repository: repository.comment_thread_repository(),
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
        ("form_id" = Option<String>, Query, format = "uuid", description = "Limit results to the specified form"),
        ("status" = Option<String>, Query, description = "Limit results to the specified answer status"),
    ),
    responses(
        AnswerSearchResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        InternalServerError,
        ServiceUnavailable,
    ),
    security(("bearer" = [])),
    tag = "Search"
)]
pub async fn search_answers(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    query: Result<Query<AnswerSearchQuery>, QueryRejection>,
) -> Result<AnswerSearchResponse, ApiError> {
    let search_use_case = SearchUseCase {
        search_repository: repository.search_repository(),
        active_form_repository: repository.active_form_repository(),
        form_answer_label_repository: repository.answer_label_repository(),
        form_label_repository: repository.form_label_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_thread_repository: repository.comment_thread_repository(),
    };

    let Query(search_query) = query.map_err_to_error().map_err(handle_error)?;
    let query = search_query
        .query
        .map(|query| query.into_inner())
        .ok_or_else(|| {
            handle_error(Error::from(PresentationError::QueryRejection {
                cause: "query is required".to_string(),
            }))
        })?;

    let answers = search_use_case
        .search_answers(&user, query, search_query.form_id, search_query.status)
        .await
        .map_err(handle_error)?;

    Ok(AnswerSearchResponse::Ok(answers.into()))
}

pub async fn start_sync(
    repository: RealInfrastructureRepository,
    receiver: Receiver<SearchableFieldsWithOperation>,
    shutdown_status: watch::Receiver<bool>,
    search_engine_initialization: watch::Receiver<SearchEngineInitializationStatus>,
) -> Result<(), Error> {
    if !wait_for_search_engine_initialization(search_engine_initialization.clone()).await {
        return Ok(());
    }
    if *search_engine_initialization.borrow() == SearchEngineInitializationStatus::Shutdown {
        return Ok(());
    }

    let search_use_case = SearchUseCase {
        search_repository: repository.search_repository(),
        active_form_repository: repository.active_form_repository(),
        form_answer_label_repository: repository.answer_label_repository(),
        form_label_repository: repository.form_label_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_thread_repository: repository.comment_thread_repository(),
    };

    search_use_case.start_sync(receiver, shutdown_status).await
}

pub async fn start_watch_out_of_sync(
    repository: RealInfrastructureRepository,
    shutdown_status: watch::Receiver<bool>,
    search_engine_initialization: watch::Receiver<SearchEngineInitializationStatus>,
) -> Result<(), Error> {
    if !wait_for_search_engine_initialization(search_engine_initialization.clone()).await {
        return Ok(());
    }
    if *search_engine_initialization.borrow() == SearchEngineInitializationStatus::Shutdown {
        return Ok(());
    }

    let search_use_case = SearchUseCase {
        search_repository: repository.search_repository(),
        active_form_repository: repository.active_form_repository(),
        form_answer_label_repository: repository.answer_label_repository(),
        form_label_repository: repository.form_label_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        comment_thread_repository: repository.comment_thread_repository(),
    };

    search_use_case
        .start_watch_out_of_sync(shutdown_status)
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
        comment_thread_repository: repository.comment_thread_repository(),
    };

    search_use_case.initialize_search_engine().await
}

pub async fn wait_for_search_engine_initialization(
    mut status: watch::Receiver<SearchEngineInitializationStatus>,
) -> bool {
    loop {
        match *status.borrow() {
            SearchEngineInitializationStatus::Pending => {}
            SearchEngineInitializationStatus::Ready => return true,
            SearchEngineInitializationStatus::Shutdown => return false,
        }

        if status.changed().await.is_err() {
            return false;
        }
    }
}

pub async fn start_initialize_search_engine(
    repository: RealInfrastructureRepository,
    status_sender: watch::Sender<SearchEngineInitializationStatus>,
    mut status: watch::Receiver<SearchEngineInitializationStatus>,
) -> Result<(), Error> {
    loop {
        let result = tokio::select! {
            _ = wait_for_search_engine_shutdown(&mut status) => {
                info!("Search engine initialization stopped by shutdown");
                return Ok(());
            }
            result = initialize_search_engine(repository.clone()) => result,
        };

        match result {
            Ok(()) => {
                if !update_search_engine_initialization_status(
                    &status_sender,
                    SearchEngineInitializationStatus::Ready,
                ) {
                    return Ok(());
                }
                info!("Search engine initialization completed");
                return Ok(());
            }
            Err(error) => {
                error!(
                    %error,
                    retry_interval_seconds = SEARCH_ENGINE_INITIALIZATION_RETRY_INTERVAL.as_secs(),
                    "failed to initialize search engine; retrying"
                );
            }
        }

        tokio::select! {
            _ = wait_for_search_engine_shutdown(&mut status) => {
                info!("Search engine initialization stopped by shutdown");
                return Ok(());
            }
            _ = tokio::time::sleep(SEARCH_ENGINE_INITIALIZATION_RETRY_INTERVAL) => {}
        }
    }
}

async fn wait_for_search_engine_shutdown(
    status: &mut watch::Receiver<SearchEngineInitializationStatus>,
) {
    loop {
        if *status.borrow() == SearchEngineInitializationStatus::Shutdown {
            return;
        }

        if status.changed().await.is_err() {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn search_engine_initialization_gate_waits_for_ready() {
        let (sender, receiver) = watch::channel(SearchEngineInitializationStatus::Pending);
        let waiter = tokio::spawn(wait_for_search_engine_initialization(receiver));

        sender
            .send(SearchEngineInitializationStatus::Ready)
            .unwrap();

        assert!(waiter.await.unwrap());
    }

    #[tokio::test]
    async fn search_engine_initialization_gate_releases_on_shutdown() {
        let (sender, receiver) = watch::channel(SearchEngineInitializationStatus::Pending);
        let waiter = tokio::spawn(wait_for_search_engine_initialization(receiver));

        sender
            .send(SearchEngineInitializationStatus::Shutdown)
            .unwrap();

        assert!(!waiter.await.unwrap());
    }

    #[test]
    fn search_engine_shutdown_status_cannot_be_overwritten_by_ready() {
        let (sender, receiver) = watch::channel(SearchEngineInitializationStatus::Pending);

        assert!(update_search_engine_initialization_status(
            &sender,
            SearchEngineInitializationStatus::Shutdown,
        ));
        assert!(!update_search_engine_initialization_status(
            &sender,
            SearchEngineInitializationStatus::Ready,
        ));
        assert_eq!(
            *receiver.borrow(),
            SearchEngineInitializationStatus::Shutdown
        );
    }
}
