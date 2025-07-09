use crate::handlers::error_handler::{handle_json_rejection, handle_path_rejection};
use axum::extract::rejection::{JsonRejection, PathRejection};
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
use resource::repository::RealInfrastructureRepository;
use usecase::forms::form_label::FormLabelUseCase;

use crate::{
    handlers::error_handler::handle_error,
    schemas::form::form_request_schemas::{FormLabelSchema, ReplaceFormLabelSchema},
};

pub async fn create_label_for_forms(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    json: Result<Json<FormLabelSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_label_use_case = FormLabelUseCase {
        form_label_repository: repository.form_label_repository(),
    };

    let Json(label) = json.map_err(handle_json_rejection)?;

    Ok(
        match form_label_use_case
            .create_label_for_forms(&user, FormLabelName::new(label.name))
            .await
        {
            Ok(_) => StatusCode::CREATED.into_response(),
            Err(err) => handle_error(err).into_response(),
        },
    )
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
    path: Result<Path<FormLabelId>, PathRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_label_use_case = FormLabelUseCase {
        form_label_repository: repository.form_label_repository(),
    };

    let Path(label_id) = path.map_err(handle_path_rejection)?;

    Ok(
        match form_label_use_case
            .delete_label_for_forms(label_id, &user)
            .await
        {
            Ok(_) => StatusCode::OK.into_response(),
            Err(err) => handle_error(err).into_response(),
        },
    )
}

pub async fn edit_label_for_forms(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormLabelId>, PathRejection>,
    json: Result<Json<FormLabelSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_label_use_case = FormLabelUseCase {
        form_label_repository: repository.form_label_repository(),
    };

    let Path(label_id) = path.map_err(handle_path_rejection)?;
    let Json(label) = json.map_err(handle_json_rejection)?;

    Ok(
        match form_label_use_case
            .edit_label_for_forms(label_id, FormLabelName::new(label.name), &user)
            .await
        {
            Ok(_) => StatusCode::OK.into_response(),
            Err(err) => handle_error(err).into_response(),
        },
    )
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

    let Path(form_id) = path.map_err(handle_path_rejection)?;
    let Json(label_ids) = json.map_err(handle_json_rejection)?;

    Ok(
        match form_label_use_case
            .replace_form_labels(&user, form_id, label_ids.labels)
            .await
        {
            Ok(_) => StatusCode::OK.into_response(),
            Err(err) => handle_error(err).into_response(),
        },
    )
}
