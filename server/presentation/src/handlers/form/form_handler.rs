use axum::extract::rejection::JsonRejection;
use axum::response::Response;
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::{HeaderValue, StatusCode, header},
    response::IntoResponse,
};
use domain::{form::models::FormId, repository::Repositories, user::models::User};
use itertools::Itertools;
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::{dto::FormDto, forms::form::FormUseCase};

use crate::handlers::error_handler::handle_error;
use crate::schemas::form::{
    form_request_schemas::{FormCreateSchema, FormUpdateSchema, OffsetAndLimit},
    form_response_schemas::{FormMetaSchema, FormSchema, FormSettingsSchema},
};
use axum::extract::rejection::PathRejection;
use domain::form::models::FormDescription;
use errors::ErrorExtra;

pub async fn create_form_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    json: Result<Json<FormCreateSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_use_case = FormUseCase {
        form_repository: repository.form_repository(),
        notification_repository: repository.notification_repository(),
        question_repository: repository.form_question_repository(),
        form_label_repository: repository.form_label_repository(),
    };

    let Json(form) = json.map_err_to_error().map_err(handle_error)?;

    let form_description = FormDescription::new(form.description);

    let id = form_use_case
        .create_form(form.title, form_description, user)
        .await
        .map_err(handle_error)?;

    Ok((
        StatusCode::CREATED,
        [(
            header::LOCATION,
            HeaderValue::from_str(id.to_string().as_str()).unwrap(),
        )],
        Json(json!({ "id": id })),
    )
        .into_response())
}

pub async fn form_list_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Query(offset_and_limit): Query<OffsetAndLimit>,
) -> Result<impl IntoResponse, Response> {
    let form_use_case = FormUseCase {
        form_repository: repository.form_repository(),
        notification_repository: repository.notification_repository(),
        question_repository: repository.form_question_repository(),
        form_label_repository: repository.form_label_repository(),
    };

    let forms = form_use_case
        .form_list(&user, offset_and_limit.offset, offset_and_limit.limit)
        .await
        .map_err(handle_error)?;

    let response_schema = forms
        .into_iter()
        .map(|(form, questions, labels)| FormSchema {
            id: form.id().to_owned(),
            title: form.title().to_owned(),
            description: form.description().to_owned(),
            settings: FormSettingsSchema::from_settings_ref(&user, form.settings()),
            metadata: FormMetaSchema::from_meta_ref(form.metadata()),
            questions,
            labels,
        })
        .collect_vec();

    Ok((StatusCode::OK, Json(response_schema)).into_response())
}

pub async fn get_form_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormId>, PathRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_use_case = FormUseCase {
        form_repository: repository.form_repository(),
        notification_repository: repository.notification_repository(),
        question_repository: repository.form_question_repository(),
        form_label_repository: repository.form_label_repository(),
    };

    let Path(form_id) = path.map_err_to_error().map_err(handle_error)?;

    let FormDto {
        form,
        questions,
        labels,
    } = form_use_case
        .get_form(&user, form_id)
        .await
        .map_err(handle_error)?;

    let response = FormSchema {
        id: form.id().to_owned(),
        title: form.title().to_owned(),
        description: form.description().to_owned(),
        settings: FormSettingsSchema::from_settings_ref(&user, form.settings()),
        metadata: FormMetaSchema::from_meta_ref(form.metadata()),
        questions,
        labels,
    };

    Ok((StatusCode::OK, Json(json!(response))).into_response())
}

pub async fn delete_form_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormId>, PathRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_use_case = FormUseCase {
        form_repository: repository.form_repository(),
        notification_repository: repository.notification_repository(),
        question_repository: repository.form_question_repository(),
        form_label_repository: repository.form_label_repository(),
    };

    let Path(form_id) = path.map_err_to_error().map_err(handle_error)?;

    form_use_case
        .delete_form(&user, form_id)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

pub async fn update_form_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormId>, PathRejection>,
    json: Result<Json<FormUpdateSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_use_case = FormUseCase {
        form_repository: repository.form_repository(),
        notification_repository: repository.notification_repository(),
        question_repository: repository.form_question_repository(),
        form_label_repository: repository.form_label_repository(),
    };

    let Path(form_id) = path.map_err_to_error().map_err(handle_error)?;
    let Json(targets) = json.map_err_to_error().map_err(handle_error)?;

    let title = targets.title;
    let description = targets.description.map(FormDescription::new);
    let (response_period, webhook_url, default_answer_title, visibility, answer_visibility) =
        if let Some(settings) = &targets.settings {
            (
                settings
                    .answer_settings
                    .as_ref()
                    .and_then(|answer_settings| answer_settings.response_period.to_owned()),
                settings.webhook_url.to_owned().and_then(|url| url.0),
                settings
                    .answer_settings
                    .as_ref()
                    .and_then(|answer_settings| answer_settings.default_answer_title.to_owned()),
                settings.visibility,
                settings
                    .answer_settings
                    .as_ref()
                    .and_then(|answer_settings| answer_settings.visibility.to_owned()),
            )
        } else {
            (None, None, None, None, None)
        };

    let (updated_form, questions, labels) = form_use_case
        .update_form(
            &user,
            form_id,
            title,
            description,
            response_period,
            webhook_url,
            default_answer_title,
            visibility,
            answer_visibility,
        )
        .await
        .map_err(handle_error)?;

    Ok((
        StatusCode::OK,
        Json(FormSchema {
            id: updated_form.id().to_owned(),
            title: updated_form.title().to_owned(),
            description: updated_form.description().to_owned(),
            settings: FormSettingsSchema::from_settings_ref(&user, updated_form.settings()),
            metadata: FormMetaSchema::from_meta_ref(updated_form.metadata()),
            questions,
            labels,
        }),
    )
        .into_response())
}
