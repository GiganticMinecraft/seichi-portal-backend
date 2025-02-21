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

use crate::{
    handlers::error_handler::handle_error,
    schemas::form::{
        form_request_schemas::{FormCreateSchema, FormUpdateSchema, OffsetAndLimit},
        form_response_schemas::{
            FormListSchema, FormMetaSchema, FormSchema, FormSettingsSchema, ResponsePeriodSchema,
        },
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
        question_repository: repository.form_question_repository(),
        form_label_repository: repository.form_label_repository(),
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
        question_repository: repository.form_question_repository(),
        form_label_repository: repository.form_label_repository(),
    };

    match form_use_case
        .form_list(&user, offset_and_limit.offset, offset_and_limit.limit)
        .await
    {
        Ok(forms) => {
            let response_schema = forms
                .into_iter()
                .map(|(form, labels)| FormListSchema {
                    id: form.id().to_owned(),
                    title: form.title().to_owned().into_inner().into_inner(),
                    description: form
                        .description()
                        .to_owned()
                        .into_inner()
                        .map(|desc| desc.to_string()),
                    response_period: ResponsePeriodSchema {
                        start_at: form
                            .settings()
                            .answer_settings()
                            .response_period()
                            .start_at()
                            .map(|start_at| start_at.to_owned()),
                        end_at: form
                            .settings()
                            .answer_settings()
                            .response_period()
                            .end_at()
                            .map(|end_at| end_at.to_owned()),
                    },
                    answer_visibility: form
                        .settings()
                        .answer_settings()
                        .visibility()
                        .to_owned()
                        .into(),
                    labels,
                })
                .collect_vec();

            (StatusCode::OK, Json(response_schema)).into_response()
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
        question_repository: repository.form_question_repository(),
        form_label_repository: repository.form_label_repository(),
    };

    match form_use_case.get_form(&user, form_id).await {
        Ok(FormDto {
            form,
            questions,
            labels,
        }) => {
            let response = FormSchema {
                id: form.id().to_owned(),
                title: form.title().to_owned(),
                description: form.description().to_owned(),
                settings: FormSettingsSchema::from_settings_ref(form.settings()),
                metadata: FormMetaSchema::from_meta_ref(form.metadata()),
                questions,
                labels,
            };

            (StatusCode::OK, Json(json!(response))).into_response()
        }
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn delete_form_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path(form_id): Path<FormId>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        form_repository: repository.form_repository(),
        notification_repository: repository.notification_repository(),
        question_repository: repository.form_question_repository(),
        form_label_repository: repository.form_label_repository(),
    };

    match form_use_case.delete_form(&user, form_id).await {
        Ok(form_id) => (StatusCode::OK, Json(json!({ "id": form_id }))).into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}

pub async fn update_form_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Path(form_id): Path<FormId>,
    Json(targets): Json<FormUpdateSchema>,
) -> impl IntoResponse {
    let form_use_case = FormUseCase {
        form_repository: repository.form_repository(),
        notification_repository: repository.notification_repository(),
        question_repository: repository.form_question_repository(),
        form_label_repository: repository.form_label_repository(),
    };

    match form_use_case
        .update_form(
            &user,
            form_id,
            targets.title,
            targets.description,
            targets.response_period,
            targets.webhook,
            targets.default_answer_title,
            targets.visibility,
            targets.answer_visibility,
        )
        .await
    {
        Ok(form) => (StatusCode::OK, Json(form)).into_response(),
        Err(err) => handle_error(err).into_response(),
    }
}
