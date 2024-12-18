use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderValue, StatusCode},
    response::IntoResponse,
    Extension, Json,
};
use domain::{form::models::FormId, repository::Repositories, user::models::User};
use itertools::Itertools;
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::forms::form::FormUseCase;

use crate::{
    handlers::error_handler::handle_error,
    schemas::form::{
        form_request_schemas::{FormCreateSchema, FormUpdateSchema, OffsetAndLimit},
        form_response_schemas::FormListSchema,
    },
};

pub async fn create_form_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Json(form): Json<FormCreateSchema>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        form_repository: repository.form_repository(),
        notification_repository: repository.notification_repository(),
    };

    match form_use_case
        .create_form(form.title, form.description, user)
        .await
    {
        Ok(id) => (
            StatusCode::CREATED,
            [(
                header::LOCATION,
                HeaderValue::from_str(id.to_string().as_str()).unwrap(),
            )],
            Json(json!({ "id": id })),
        )
            .into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn form_list_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Query(offset_and_limit): Query<OffsetAndLimit>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        form_repository: repository.form_repository(),
        notification_repository: repository.notification_repository(),
    };

    match form_use_case
        .form_list(&user, offset_and_limit.offset, offset_and_limit.limit)
        .await
    {
        Ok(forms) => {
            let form_list_schema = forms
                .into_iter()
                .map(Into::<FormListSchema>::into)
                .collect_vec();

            (StatusCode::OK, Json(form_list_schema)).into_response()
        }
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn get_form_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path(form_id): Path<FormId>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        form_repository: repository.form_repository(),
        notification_repository: repository.notification_repository(),
    };

    // FIXME: forms から questions を剥がしたので、usecase で questions を取得する必要がある
    match form_use_case.get_form(&user, form_id).await {
        Ok(form) => (StatusCode::OK, Json(form)).into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn delete_form_handler(
    State(repository): State<RealInfrastructureRepository>,
    Path(form_id): Path<FormId>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        form_repository: repository.form_repository(),
        notification_repository: repository.notification_repository(),
    };

    match form_use_case.delete_form(form_id).await {
        Ok(form_id) => (StatusCode::OK, Json(json!({ "id": form_id }))).into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn update_form_handler(
    State(repository): State<RealInfrastructureRepository>,
    Path(form_id): Path<FormId>,
    Json(targets): Json<FormUpdateSchema>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        form_repository: repository.form_repository(),
        notification_repository: repository.notification_repository(),
    };

    match form_use_case
        .update_form(
            &form_id,
            targets.title.as_ref(),
            targets.description.as_ref(),
            targets.has_response_period,
            targets.response_period.as_ref(),
            targets.webhook.as_ref(),
            targets.default_answer_title.as_ref(),
            targets.visibility.as_ref(),
            targets.answer_visibility.as_ref(),
        )
        .await
    {
        Ok(form) => (StatusCode::OK, Json(form)).into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}
