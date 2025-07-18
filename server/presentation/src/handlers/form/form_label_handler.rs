use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::http::{HeaderValue, header};
use axum::response::Response;
use axum::{
    Extension,
    extract::{Json, Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use domain::{
    form::models::{FormId, FormLabelId, FormLabelName},
    repository::Repositories,
    user::models::User,
};
use errors::ErrorExtra;
use resource::repository::RealInfrastructureRepository;
use usecase::forms::form_label::FormLabelUseCase;

use crate::schemas::form::form_request_schemas::FormLabelCreateSchema;
use crate::{
    handlers::error_handler::handle_error,
    schemas::form::form_request_schemas::{FormLabelUpdateSchema, ReplaceFormLabelSchema},
};

pub async fn create_label_for_forms(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    json: Result<Json<FormLabelCreateSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_label_use_case = FormLabelUseCase {
        form_label_repository: repository.form_label_repository(),
    };

    let Json(label) = json.map_err_to_error().map_err(handle_error)?;

    let created_label = form_label_use_case
        .create_label_for_forms(&user, FormLabelName::new(label.name))
        .await
        .map_err(handle_error)?;

    Ok((
        StatusCode::CREATED,
        [(
            header::LOCATION,
            HeaderValue::from_str(
                created_label
                    .id()
                    .to_owned()
                    .into_inner()
                    .to_string()
                    .as_str(),
            )
            .unwrap(),
        )],
        Json(created_label),
    )
        .into_response())
}

pub async fn get_labels_for_forms(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
) -> Result<impl IntoResponse, Response> {
    let form_label_use_case = FormLabelUseCase {
        form_label_repository: repository.form_label_repository(),
    };

    let labels = form_label_use_case
        .get_labels_for_forms(&user)
        .await
        .map_err(handle_error)?;
    Ok((StatusCode::OK, Json(labels)).into_response())
}

pub async fn delete_label_for_forms(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormLabelId>, PathRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_label_use_case = FormLabelUseCase {
        form_label_repository: repository.form_label_repository(),
    };

    let Path(label_id) = path.map_err_to_error().map_err(handle_error)?;

    form_label_use_case
        .delete_label_for_forms(label_id, &user)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::OK.into_response())
}

pub async fn edit_label_for_forms(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormLabelId>, PathRejection>,
    json: Result<Json<FormLabelUpdateSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_label_use_case = FormLabelUseCase {
        form_label_repository: repository.form_label_repository(),
    };

    let Path(label_id) = path.map_err_to_error().map_err(handle_error)?;
    let Json(label) = json.map_err_to_error().map_err(handle_error)?;

    form_label_use_case
        .edit_label_for_forms(label_id, label.name.map(FormLabelName::new), &user)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::OK.into_response())
}

pub async fn replace_form_labels(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormId>, PathRejection>,
    json: Result<Json<ReplaceFormLabelSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_label_use_case = FormLabelUseCase {
        form_label_repository: repository.form_label_repository(),
    };

    let Path(form_id) = path.map_err_to_error().map_err(handle_error)?;
    let Json(label_ids) = json.map_err_to_error().map_err(handle_error)?;

    form_label_use_case
        .replace_form_labels(&user, form_id, label_ids.labels)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::OK.into_response())
}
