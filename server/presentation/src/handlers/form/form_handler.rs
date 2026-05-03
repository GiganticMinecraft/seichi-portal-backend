use axum::extract::rejection::JsonRejection;
use axum::response::Response;
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::{HeaderValue, StatusCode, header},
    response::IntoResponse,
};
use domain::{
    form::{models::FormId, question::models::QuestionSet},
    repository::Repositories,
    user::models::User,
};
use itertools::Itertools;
use resource::repository::RealInfrastructureRepository;
use serde_json::json;
use usecase::{
    dto::{FormDto, UpsertQuestionDto},
    forms::form::FormUseCase,
};

use crate::handlers::error_handler::handle_error;
use crate::schemas::error_responses::*;
use crate::schemas::form::{
    form_request_schemas::{FormCreateSchema, FormUpdateSchema, OffsetAndLimit, QuestionSchema},
    form_response_schemas::{
        FormMetaSchema, FormSchema, FormSettingsSchema, QuestionResponseSchema,
    },
};
use axum::extract::rejection::PathRejection;
use domain::form::models::FormDescription;
use errors::ErrorExtra;
use types::non_empty_vec::NonEmptyVec;

#[derive(utoipa::IntoResponses)]
pub enum CreateFormResponse {
    #[response(
        status = 201,
        description = "The request has succeeded and a new resource has been created as a result."
    )]
    Created(FormSchema),
}

impl IntoResponse for CreateFormResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Created(body) => (
                StatusCode::CREATED,
                [(
                    header::LOCATION,
                    HeaderValue::from_str(body.id.to_owned().into_inner().to_string().as_str())
                        .unwrap(),
                )],
                Json(json!(body)),
            )
                .into_response(),
        }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum FormListResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(Vec<FormSchema>),
}

impl IntoResponse for FormListResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum GetFormResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(FormSchema),
}

impl IntoResponse for GetFormResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(json!(body))).into_response(),
        }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum UpdateFormResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(FormSchema),
}

impl IntoResponse for UpdateFormResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

#[utoipa::path(
    post,
    path = "/forms",
    summary = "フォームの作成",
    request_body = FormCreateSchema,
    responses(
        CreateFormResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Forms"
)]
pub async fn create_form_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    json: Result<Json<FormCreateSchema>, JsonRejection>,
) -> Result<CreateFormResponse, Response> {
    let form_use_case = FormUseCase {
        form_repository: repository.form_repository(),
        notification_repository: repository.notification_repository(),
        question_repository: repository.form_question_repository(),
        form_label_repository: repository.form_label_repository(),
        answer_repository: repository.form_answer_repository(),
    };

    let Json(form) = json.map_err_to_error().map_err(handle_error)?;

    let form_description = FormDescription::new(form.description);

    let form = form_use_case
        .create_form(form.title, form_description, &user)
        .await
        .map_err(handle_error)?;

    Ok(CreateFormResponse::Created(FormSchema {
        id: form.id().to_owned(),
        title: form.title().to_owned(),
        description: form.description().to_owned(),
        settings: FormSettingsSchema::from_settings_ref(&user, form.settings()),
        metadata: FormMetaSchema::from_meta_ref(form.metadata()),
        questions: vec![],
        labels: vec![],
    }))
}

#[utoipa::path(
    get,
    path = "/forms",
    summary = "フォームの一覧取得",
    params(
        ("offset" = Option<u32>, Query, description = "Offset for pagination"),
        ("limit" = Option<u32>, Query, description = "Limit for pagination"),
    ),
    responses(
        FormListResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Forms"
)]
pub async fn form_list_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Query(offset_and_limit): Query<OffsetAndLimit>,
) -> Result<FormListResponse, Response> {
    let form_use_case = FormUseCase {
        form_repository: repository.form_repository(),
        notification_repository: repository.notification_repository(),
        question_repository: repository.form_question_repository(),
        form_label_repository: repository.form_label_repository(),
        answer_repository: repository.form_answer_repository(),
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
            questions: questions
                .into_iter()
                .map(QuestionResponseSchema::from)
                .collect(),
            labels,
        })
        .collect_vec();

    Ok(FormListResponse::Ok(response_schema))
}

#[utoipa::path(
    get,
    path = "/forms/{id}",
    summary = "フォームの取得",
    params(
        ("id" = String, Path, description = "Form ID"),
    ),
    responses(
        GetFormResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Forms"
)]
pub async fn get_form_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormId>, PathRejection>,
) -> Result<GetFormResponse, Response> {
    let form_use_case = FormUseCase {
        form_repository: repository.form_repository(),
        notification_repository: repository.notification_repository(),
        question_repository: repository.form_question_repository(),
        form_label_repository: repository.form_label_repository(),
        answer_repository: repository.form_answer_repository(),
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

    Ok(GetFormResponse::Ok(FormSchema {
        id: form.id().to_owned(),
        title: form.title().to_owned(),
        description: form.description().to_owned(),
        settings: FormSettingsSchema::from_settings_ref(&user, form.settings()),
        metadata: FormMetaSchema::from_meta_ref(form.metadata()),
        questions: questions
            .into_iter()
            .map(QuestionResponseSchema::from)
            .collect(),
        labels,
    }))
}

#[utoipa::path(
    delete,
    path = "/forms/{id}",
    summary = "フォームの削除",
    params(
        ("id" = String, Path, description = "Form ID"),
    ),
    responses(
        (status = 204, description = "There is no content to send for this request, but the headers may be useful."),
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Forms"
)]
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
        answer_repository: repository.form_answer_repository(),
    };

    let Path(form_id) = path.map_err_to_error().map_err(handle_error)?;

    form_use_case
        .delete_form(&user, form_id)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

#[utoipa::path(
    put,
    path = "/forms/{id}",
    summary = "フォームの更新",
    description = "questions を含めた場合、その form 配下の question 定義全体を指定内容で置換します。questions を省略した場合は既存 question を保持します。",
    params(
        ("id" = String, Path, description = "Form ID"),
    ),
    request_body = FormUpdateSchema,
    responses(
        UpdateFormResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Forms"
)]
pub async fn update_form_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormId>, PathRejection>,
    json: Result<Json<FormUpdateSchema>, JsonRejection>,
) -> Result<UpdateFormResponse, Response> {
    let form_use_case = FormUseCase {
        form_repository: repository.form_repository(),
        notification_repository: repository.notification_repository(),
        question_repository: repository.form_question_repository(),
        form_label_repository: repository.form_label_repository(),
        answer_repository: repository.form_answer_repository(),
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
    let questions = targets
        .questions
        .map(|questions| into_upsert_question_dtos(form_id, questions))
        .transpose()
        .map_err(errors::Error::from)
        .map_err(handle_error)?;

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
            questions,
        )
        .await
        .map_err(handle_error)?;

    Ok(UpdateFormResponse::Ok(FormSchema {
        id: updated_form.id().to_owned(),
        title: updated_form.title().to_owned(),
        description: updated_form.description().to_owned(),
        settings: FormSettingsSchema::from_settings_ref(&user, updated_form.settings()),
        metadata: FormMetaSchema::from_meta_ref(updated_form.metadata()),
        questions: questions
            .into_iter()
            .map(QuestionResponseSchema::from)
            .collect(),
        labels,
    }))
}

fn into_upsert_question_dtos(
    form_id: FormId,
    questions: Vec<QuestionSchema>,
) -> Result<Vec<UpsertQuestionDto>, errors::domain::DomainError> {
    let questions = questions
        .into_iter()
        .map(|question| into_upsert_question_dto(form_id, question))
        .collect::<Result<Vec<_>, _>>()?;

    QuestionSet::try_new(
        questions
            .iter()
            .map(|question| question.question.clone())
            .collect(),
    )?;

    Ok(questions)
}

fn into_upsert_question_dto(
    form_id: FormId,
    question: QuestionSchema,
) -> Result<UpsertQuestionDto, errors::domain::DomainError> {
    let (question_type, definition, choices) = question.into_parts();
    let original_id = definition.id;
    let choices = into_domain_choices(choices)?;
    let question = match original_id {
        Some(question_id) => domain::form::question::models::Question::from_raw_parts(
            question_id,
            form_id,
            definition.template_key,
            definition.position,
            definition.title,
            definition.description,
            question_type,
            choices,
            definition.is_required,
        )?,
        None => match question_type {
            domain::form::question::models::QuestionType::Text => {
                domain::form::question::models::Question::new_text(
                    form_id,
                    definition.template_key,
                    definition.position,
                    definition.title,
                    definition.description,
                    definition.is_required,
                )?
            }
            domain::form::question::models::QuestionType::SingleChoice => {
                domain::form::question::models::Question::new_single_choice(
                    form_id,
                    definition.template_key,
                    definition.position,
                    definition.title,
                    definition.description,
                    required_choices(choices)?,
                    definition.is_required,
                )?
            }
            domain::form::question::models::QuestionType::MultipleChoice => {
                domain::form::question::models::Question::new_multiple_choice(
                    form_id,
                    definition.template_key,
                    definition.position,
                    definition.title,
                    definition.description,
                    required_choices(choices)?,
                    definition.is_required,
                )?
            }
        },
    };

    Ok(UpsertQuestionDto {
        original_id,
        question,
    })
}

fn into_domain_choices(
    choices: Option<Vec<crate::schemas::form::form_request_schemas::ChoiceSchema>>,
) -> Result<Option<NonEmptyVec<domain::form::question::models::Choice>>, errors::domain::DomainError>
{
    let Some(choices) = choices else {
        return Ok(None);
    };

    let choices = choices
        .into_iter()
        .map(|choice| {
            domain::form::question::models::Choice::new(choice.id, choice.position, choice.label)
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok((!choices.is_empty()).then(|| NonEmptyVec::try_new(choices).expect("non-empty choices")))
}

fn required_choices(
    choices: Option<NonEmptyVec<domain::form::question::models::Choice>>,
) -> Result<NonEmptyVec<domain::form::question::models::Choice>, errors::domain::DomainError> {
    choices.ok_or_else(|| errors::domain::DomainError::InvalidEntity {
        message: "choice question must have at least one choice".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::form::question::models::QuestionType;
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn deserialize_and_convert_text_question_variant() {
        let question: QuestionSchema = serde_json::from_value(json!({
            "question_type": "Text",
            "template_key": "body",
            "position": 0,
            "title": "Body",
            "description": "desc",
            "is_required": true
        }))
        .unwrap();

        let result = into_upsert_question_dto(FormId::from(Uuid::nil()), question).unwrap();

        assert_eq!(result.question.question_type(), QuestionType::Text);
        assert!(result.question.choices().is_none());
    }

    #[test]
    fn deserialize_and_convert_multiple_choice_variant() {
        let question: QuestionSchema = serde_json::from_value(json!({
            "question_type": "MultipleChoice",
            "template_key": "roles",
            "position": 0,
            "title": "Roles",
            "description": "desc",
            "is_required": false,
            "choices": [
                { "position": 0, "label": "Admin" },
                { "position": 1, "label": "User" }
            ]
        }))
        .unwrap();

        let result = into_upsert_question_dto(FormId::from(Uuid::nil()), question).unwrap();

        assert_eq!(
            result.question.question_type(),
            QuestionType::MultipleChoice
        );
        assert_eq!(result.question.choices().unwrap().len(), 2);
    }

    #[test]
    fn text_question_with_choices_is_rejected_during_deserialization() {
        let result = serde_json::from_value::<QuestionSchema>(json!({
            "question_type": "Text",
            "template_key": "body",
            "position": 0,
            "title": "Body",
            "is_required": true,
            "choices": [
                { "position": 0, "label": "unexpected" }
            ]
        }));

        assert!(result.is_err());
    }

    #[test]
    fn choice_question_without_choices_is_rejected() {
        let question: QuestionSchema = serde_json::from_value(json!({
            "question_type": "SingleChoice",
            "template_key": "role",
            "position": 0,
            "title": "Role",
            "is_required": true,
            "choices": []
        }))
        .unwrap();

        let result = into_upsert_question_dto(FormId::from(Uuid::nil()), question);

        assert!(matches!(
            result,
            Err(errors::domain::DomainError::InvalidEntity { .. })
        ));
    }

    #[test]
    fn duplicate_positions_are_rejected_for_replacement_payload() {
        let questions: Vec<QuestionSchema> = serde_json::from_value(json!([
            {
                "question_type": "Text",
                "template_key": "body",
                "position": 0,
                "title": "Body",
                "is_required": true
            },
            {
                "question_type": "Text",
                "template_key": "summary",
                "position": 0,
                "title": "Summary",
                "is_required": true
            }
        ]))
        .unwrap();

        let result = into_upsert_question_dtos(FormId::from(Uuid::nil()), questions);

        assert!(matches!(
            result,
            Err(errors::domain::DomainError::InvalidEntity { .. })
        ));
    }
}
