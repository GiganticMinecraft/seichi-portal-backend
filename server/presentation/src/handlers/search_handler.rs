use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use domain::repository::Repositories;
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::search::SearchUseCase;

use crate::{handlers::error_handler::handle_error, schemas::search_schemas::SearchQuery};

pub async fn cross_search(
    State(repository): State<RealInfrastructureRepository>,
    Query(search_query): Query<SearchQuery>,
) -> impl IntoResponse {
    let search_use_case = SearchUseCase {
        repository: repository.search_repository(),
    };

    match search_query {
        SearchQuery { query: None } => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "reason": "query is required" })),
        )
            .into_response(),
        SearchQuery { query: Some(query) } => match search_use_case.cross_search(query).await {
            Ok(result) => (StatusCode::OK, Json(result)).into_response(),
            Err(err) => handle_error(err).into_response(),
        },
    }
}
