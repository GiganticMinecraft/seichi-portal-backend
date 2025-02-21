use axum::{
    Extension, Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use domain::{repository::Repositories, user::models::User};
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::search::SearchUseCase;

use crate::{
    handlers::error_handler::handle_error,
    schemas::search_schemas::{CrossSearchResult, SearchQuery},
};

pub async fn cross_search(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Query(search_query): Query<SearchQuery>,
) -> impl IntoResponse {
    let search_use_case = SearchUseCase {
        repository: repository.search_repository(),
        answer_repository: repository.form_answer_repository(),
        form_repository: repository.form_repository(),
    };

    match search_query {
        SearchQuery { query: None } => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "reason": "query is required" })),
        )
            .into_response(),
        SearchQuery { query: Some(query) } => {
            match search_use_case.cross_search(&user, query).await {
                Ok(result) => {
                    (StatusCode::OK, Json(json!(CrossSearchResult::from(result)))).into_response()
                }
                Err(err) => handle_error(err).into_response(),
            }
        }
    }
}
