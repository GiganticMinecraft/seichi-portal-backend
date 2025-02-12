use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Extension, Json,
};
use domain::{
    form::models::{FormId, FormLabelId, FormLabelName},
    repository::Repositories,
    user::models::User,
};
use resource::repository::RealInfrastructureRepository;
use usecase::forms::form_label::FormLabelUseCase;

use crate::{
    handlers::error_handler::handle_error,
    schemas::form::form_request_schemas::{FormLabelSchema, ReplaceFormLabelSchema},
};

pub async fn create_label_for_forms(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Json(label): Json<FormLabelSchema>,
) -> impl IntoResponse {
    let form_label_use_case = FormLabelUseCase {
        form_label_repository: repository.form_label_repository(),
    };

    match form_label_use_case
        .create_label_for_forms(&user, FormLabelName::new(label.name))
        .await
    {
        Ok(_) => StatusCode::CREATED.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn get_labels_for_forms(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
) -> impl IntoResponse {
    let form_label_use_case = FormLabelUseCase {
        form_label_repository: repository.form_label_repository(),
    };

    match form_label_use_case.get_labels_for_forms(&user).await {
        Ok(labels) => (StatusCode::OK, Json(labels)).into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn delete_label_for_forms(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path(label_id): Path<FormLabelId>,
) -> impl IntoResponse {
    let form_label_use_case = FormLabelUseCase {
        form_label_repository: repository.form_label_repository(),
    };

    match form_label_use_case
        .delete_label_for_forms(label_id, &user)
        .await
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn edit_label_for_forms(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path(label_id): Path<FormLabelId>,
    Json(label): Json<FormLabelSchema>,
) -> impl IntoResponse {
    let form_label_use_case = FormLabelUseCase {
        form_label_repository: repository.form_label_repository(),
    };

    match form_label_use_case
        .edit_label_for_forms(label_id, FormLabelName::new(label.name), &user)
        .await
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn replace_form_labels(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path(form_id): Path<FormId>,
    Json(label_ids): Json<ReplaceFormLabelSchema>,
) -> impl IntoResponse {
    let form_label_use_case = FormLabelUseCase {
        form_label_repository: repository.form_label_repository(),
    };

    match form_label_use_case
        .replace_form_labels(&user, form_id, label_ids.labels)
        .await
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}
