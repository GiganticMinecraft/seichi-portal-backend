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
    repository::Repositories, search::models::SearchableFieldsWithOperation, user::models::User,
};
use errors::{Error, ErrorExtra};
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use tokio::sync::{Notify, mpsc::Receiver};
use usecase::search::SearchUseCase;

use crate::{
    handlers::error_handler::handle_error,
    schemas::search_schemas::{CrossSearchResult, SearchQuery},
};

pub async fn cross_search(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    query: Result<Query<SearchQuery>, QueryRejection>,
) -> Result<impl IntoResponse, Response> {
    let search_use_case = SearchUseCase {
        search_repository: repository.search_repository(),
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
        comment_repository: repository.form_comment_repository(),
        form_answer_label_repository: repository.answer_label_repository(),
        form_label_repository: repository.form_label_repository(),
        user_repository: repository.user_repository(),
    };

    let Query(search_query) = query.map_err_to_error().map_err(handle_error)?;

    match search_query {
        SearchQuery { query: None } => Ok((
            StatusCode::BAD_REQUEST,
            Json(json!({
                "errorCode": "BAD_REQUEST",
                "reason": "query is required"
            })),
        )
            .into_response()),
        SearchQuery { query: Some(query) } => {
            let result = search_use_case
                .cross_search(&user, query.into_inner())
                .await
                .map_err(handle_error)?;
            Ok((StatusCode::OK, Json(json!(CrossSearchResult::from(result)))).into_response())
        }
    }
}

pub async fn start_sync(
    repository: RealInfrastructureRepository,
    receiver: Receiver<SearchableFieldsWithOperation>,
    shutdown_notifier: Arc<Notify>,
) -> Result<(), Error> {
    let search_use_case = SearchUseCase {
        search_repository: repository.search_repository(),
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
        comment_repository: repository.form_comment_repository(),
        form_answer_label_repository: repository.answer_label_repository(),
        form_label_repository: repository.form_label_repository(),
        user_repository: repository.user_repository(),
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
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
        comment_repository: repository.form_comment_repository(),
        form_answer_label_repository: repository.answer_label_repository(),
        form_label_repository: repository.form_label_repository(),
        user_repository: repository.user_repository(),
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
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
        comment_repository: repository.form_comment_repository(),
        form_answer_label_repository: repository.answer_label_repository(),
        form_label_repository: repository.form_label_repository(),
        user_repository: repository.user_repository(),
    };

    search_use_case.initialize_search_engine().await
}
