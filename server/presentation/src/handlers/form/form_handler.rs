use axum::extract::rejection::JsonRejection;
use axum::extract::rejection::PathRejection;
use axum::response::Response;
use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::{HeaderValue, StatusCode, header},
    response::IntoResponse,
};
use domain::{
    form::{
        answer_entry_set::models::AnswerEntrySet,
        models::{ActiveForm, ArchivedForm, FormDescription, FormId, FormLabel},
        question::models::{Choice, Question, QuestionSet, QuestionType},
    },
    repository::{Repositories, form::answer_entry_set_repository::AnswerEntrySetRepository},
    user::models::{ActiveUser, Actor, User},
};
use errors::{Error, ErrorExtra, domain::DomainError};
use futures::StreamExt;
use resource::{
    database::connection::ConnectionPool,
    repository::{RealInfrastructureRepository, Repository},
};
use serde_json::json;
use types::non_empty_vec::NonEmptyVec;
use usecase::{
    forms::form::FormUseCase,
    models::{ActiveFormWithLabels, ArchivedFormDetails, UpsertQuestionInput},
};

use crate::handlers::error_handler::handle_error;
use crate::schemas::{
    error_responses::*,
    form::{
        form_request_schemas::{
            ChoiceSchema, FormCreateSchema, FormUpdateSchema, OffsetAndLimit, QuestionSchema,
        },
        form_response_schemas::{
            ArchivedFormSchema, FormMetaSchema, FormSchema, FormSettingsSchema,
            QuestionResponseSchema,
        },
    },
    search_schemas::SearchQuery,
};

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

#[derive(utoipa::IntoResponses)]
pub enum ArchivedFormListResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(Vec<ArchivedFormSchema>),
}

impl IntoResponse for ArchivedFormListResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum ArchivedFormResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(ArchivedFormSchema),
}

impl IntoResponse for ArchivedFormResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(json!(body))).into_response(),
        }
    }
}

type ResourceRepository = Repository<ConnectionPool>;
type ResourceFormUseCase<'a> = FormUseCase<
    'a,
    ResourceRepository,
    ResourceRepository,
    ResourceRepository,
    ResourceRepository,
    ResourceRepository,
    ResourceRepository,
>;

fn build_form_use_case(repository: &RealInfrastructureRepository) -> ResourceFormUseCase<'_> {
    FormUseCase {
        active_form_repository: repository.active_form_repository(),
        archived_form_repository: repository.archived_form_repository(),
        notification_repository: repository.notification_repository(),
        form_label_repository: repository.form_label_repository(),
        answer_entry_set_repository: repository.answer_entry_set_repository(),
        user_repository: repository.user_repository(),
    }
}

async fn fetch_answer_entry_set(
    repository: &RealInfrastructureRepository,
    actor: &Actor,
    form: &ActiveForm,
) -> Result<AnswerEntrySet, Error> {
    repository
        .answer_entry_set_repository()
        .get(*form.answer_entry_set_id())
        .await?
        .ok_or(errors::usecase::UseCaseError::FormNotFound)?
        .try_into_read(actor)
        .map_err(Into::into)
}

fn form_schema(
    actor: &Actor,
    form: &ActiveForm,
    answer_entry_set: &AnswerEntrySet,
    labels: Vec<FormLabel>,
) -> FormSchema {
    FormSchema {
        id: form.id().to_owned(),
        title: form.title().to_owned(),
        description: form.description().to_owned(),
        settings: FormSettingsSchema::from_settings_and_entry_set(
            actor,
            form.settings(),
            answer_entry_set,
        ),
        metadata: FormMetaSchema::from_meta_ref(form.metadata()),
        questions: form
            .questions()
            .iter()
            .cloned()
            .map(QuestionResponseSchema::from)
            .collect(),
        labels,
    }
}

fn archived_form_schema_from_parts(
    actor: &Actor,
    form: ArchivedForm,
    answer_entry_set: &AnswerEntrySet,
    archived_by: ActiveUser,
    labels: Vec<FormLabel>,
) -> ArchivedFormSchema {
    ArchivedFormSchema {
        id: form.form().id().to_owned(),
        title: form.form().title().to_owned(),
        description: form.form().description().to_owned(),
        settings: FormSettingsSchema::from_settings_and_entry_set(
            actor,
            form.form().settings(),
            answer_entry_set,
        ),
        metadata: FormMetaSchema::from_meta_ref(form.form().metadata()),
        archived_at: *form.archived_at(),
        archived_by,
        questions: form
            .form()
            .questions()
            .iter()
            .cloned()
            .map(QuestionResponseSchema::from)
            .collect(),
        labels,
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
    Extension(user): Extension<ActiveUser>,
    State(repository): State<RealInfrastructureRepository>,
    json: Result<Json<FormCreateSchema>, JsonRejection>,
) -> Result<CreateFormResponse, Response> {
    let form_use_case = build_form_use_case(&repository);

    let Json(form) = json.map_err_to_error().map_err(handle_error)?;
    let FormCreateSchema {
        title,
        description,
        settings,
        questions,
    } = form;

    let form_description = FormDescription::new(description);
    let questions = into_create_questions(questions)
        .map_err(errors::Error::from)
        .map_err(handle_error)?;

    let form = form_use_case
        .create_form(
            title,
            form_description,
            questions,
            settings.and_then(|settings| settings.allow_temporary_answers),
            &user,
        )
        .await
        .map_err(handle_error)?;

    let actor = Actor::from(user.clone());
    let answer_entry_set = fetch_answer_entry_set(&repository, &actor, &form)
        .await
        .map_err(handle_error)?;

    Ok(CreateFormResponse::Created(form_schema(
        &actor,
        &form,
        &answer_entry_set,
        vec![],
    )))
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
    security((), ("bearer" = [])),
    tag = "Forms"
)]
pub async fn form_list_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    Query(offset_and_limit): Query<OffsetAndLimit>,
) -> Result<FormListResponse, Response> {
    let form_use_case = build_form_use_case(&repository);

    let forms = form_use_case
        .form_list(
            &Actor::from(user.clone()),
            offset_and_limit.offset,
            offset_and_limit.limit,
        )
        .await
        .map_err(handle_error)?;

    let actor = Actor::from(user.clone());
    let response_schema = futures::stream::iter(forms)
        .then(|(form, labels)| {
            let repository = repository.clone();
            let actor = actor.clone();
            async move {
                let answer_entry_set = fetch_answer_entry_set(&repository, &actor, &form)
                    .await
                    .map_err(handle_error)?;
                Ok::<_, Response>(form_schema(&actor, &form, &answer_entry_set, labels))
            }
        })
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

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
    security((), ("bearer" = [])),
    tag = "Forms"
)]
pub async fn get_form_handler(
    Extension(user): Extension<User>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormId>, PathRejection>,
) -> Result<GetFormResponse, Response> {
    let form_use_case = build_form_use_case(&repository);

    let Path(form_id) = path.map_err_to_error().map_err(handle_error)?;

    let ActiveFormWithLabels { form, labels } = form_use_case
        .get_form(&Actor::from(user.clone()), form_id)
        .await
        .map_err(handle_error)?;

    let actor = Actor::from(user.clone());
    let answer_entry_set = fetch_answer_entry_set(&repository, &actor, &form)
        .await
        .map_err(handle_error)?;

    Ok(GetFormResponse::Ok(form_schema(
        &actor,
        &form,
        &answer_entry_set,
        labels,
    )))
}

#[utoipa::path(
    post,
    path = "/forms/{id}/archive",
    summary = "フォームのアーカイブ",
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
pub async fn archive_form_handler(
    Extension(user): Extension<ActiveUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormId>, PathRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_use_case = build_form_use_case(&repository);

    let Path(form_id) = path.map_err_to_error().map_err(handle_error)?;

    let archived_form = form_use_case
        .archive_form(&user, form_id)
        .await
        .map_err(handle_error)?;
    let actor = Actor::from(user.clone());
    let answer_entry_set = fetch_answer_entry_set(&repository, &actor, archived_form.form())
        .await
        .map_err(handle_error)?;

    Ok((
        StatusCode::OK,
        Json(archived_form_schema_from_parts(
            &actor,
            archived_form,
            &answer_entry_set,
            user,
            vec![],
        )),
    )
        .into_response())
}

#[utoipa::path(
    put,
    path = "/forms/{id}",
    summary = "フォームの更新",
    description = "questions または labels を含めた場合、その form 配下の値全体を指定内容で置換します。省略した場合は既存値を保持します。",
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
    Extension(user): Extension<ActiveUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormId>, PathRejection>,
    json: Result<Json<FormUpdateSchema>, JsonRejection>,
) -> Result<UpdateFormResponse, Response> {
    let form_use_case = build_form_use_case(&repository);

    let Path(form_id) = path.map_err_to_error().map_err(handle_error)?;
    let Json(targets) = json.map_err_to_error().map_err(handle_error)?;

    let title = targets.title;
    let description = targets.description.map(FormDescription::new);
    let (
        response_period,
        webhook_url,
        default_answer_title,
        visibility,
        allow_temporary_answers,
        answer_visibility,
    ) = if let Some(settings) = &targets.settings {
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
            settings.allow_temporary_answers,
            settings
                .answer_settings
                .as_ref()
                .and_then(|answer_settings| answer_settings.visibility.to_owned()),
        )
    } else {
        (None, None, None, None, None, None)
    };
    let questions = targets
        .questions
        .map(into_upsert_question_inputs)
        .transpose()
        .map_err(handle_error)?;
    let labels = targets.labels;

    let (updated_form, labels) = form_use_case
        .update_form(
            &user,
            form_id,
            title,
            description,
            response_period,
            webhook_url,
            default_answer_title,
            visibility,
            allow_temporary_answers,
            answer_visibility,
            questions,
            labels,
        )
        .await
        .map_err(handle_error)?;

    let actor = Actor::from(user.clone());
    let answer_entry_set = fetch_answer_entry_set(&repository, &actor, &updated_form)
        .await
        .map_err(handle_error)?;

    Ok(UpdateFormResponse::Ok(form_schema(
        &actor,
        &updated_form,
        &answer_entry_set,
        labels,
    )))
}

#[utoipa::path(
    get,
    path = "/archived-forms",
    summary = "アーカイブ済みフォームの一覧取得",
    params(
        ("offset" = Option<u32>, Query, description = "Offset for pagination"),
        ("limit" = Option<u32>, Query, description = "Limit for pagination"),
        ("query" = Option<String>, Query, description = "Search query"),
    ),
    responses(
        ArchivedFormListResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Archived Forms"
)]
pub async fn archived_form_list_handler(
    Extension(user): Extension<ActiveUser>,
    State(repository): State<RealInfrastructureRepository>,
    Query(offset_and_limit): Query<OffsetAndLimit>,
    query: Result<Query<SearchQuery>, axum::extract::rejection::QueryRejection>,
) -> Result<ArchivedFormListResponse, Response> {
    let form_use_case = build_form_use_case(&repository);
    let query = query
        .map_err_to_error()
        .map_err(handle_error)?
        .query
        .clone();

    let forms = form_use_case
        .archived_form_list(
            &user,
            offset_and_limit.offset,
            offset_and_limit.limit,
            query.map(|q| q.into_inner()),
        )
        .await
        .map_err(handle_error)?;

    let actor = Actor::from(user);
    let response_schema = futures::stream::iter(forms)
        .then(|details| {
            let repository = repository.clone();
            let actor = actor.clone();
            async move {
                let answer_entry_set =
                    fetch_answer_entry_set(&repository, &actor, details.form.form())
                        .await
                        .map_err(handle_error)?;
                Ok::<_, Response>(archived_form_schema_from_parts(
                    &actor,
                    details.form,
                    &answer_entry_set,
                    details.archived_by,
                    details.labels,
                ))
            }
        })
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ArchivedFormListResponse::Ok(response_schema))
}

#[utoipa::path(
    get,
    path = "/archived-forms/{id}",
    summary = "アーカイブ済みフォームの取得",
    params(
        ("id" = String, Path, description = "Form ID"),
    ),
    responses(
        ArchivedFormResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Archived Forms"
)]
pub async fn get_archived_form_handler(
    Extension(user): Extension<ActiveUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormId>, PathRejection>,
) -> Result<ArchivedFormResponse, Response> {
    let form_use_case = build_form_use_case(&repository);
    let Path(form_id) = path.map_err_to_error().map_err(handle_error)?;

    let ArchivedFormDetails {
        form,
        archived_by,
        labels,
    } = form_use_case
        .get_archived_form(&user, form_id)
        .await
        .map_err(handle_error)?;

    let actor = Actor::from(user.clone());
    let answer_entry_set = fetch_answer_entry_set(&repository, &actor, form.form())
        .await
        .map_err(handle_error)?;

    Ok(ArchivedFormResponse::Ok(archived_form_schema_from_parts(
        &actor,
        form,
        &answer_entry_set,
        archived_by,
        labels,
    )))
}

#[utoipa::path(
    post,
    path = "/archived-forms/{id}/restore",
    summary = "アーカイブ済みフォームの復元",
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
    tag = "Archived Forms"
)]
pub async fn restore_archived_form_handler(
    Extension(user): Extension<ActiveUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormId>, PathRejection>,
) -> Result<impl IntoResponse, Response> {
    let form_use_case = build_form_use_case(&repository);
    let Path(form_id) = path.map_err_to_error().map_err(handle_error)?;

    form_use_case
        .restore_form(&user, form_id)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::NO_CONTENT.into_response())
}

fn into_upsert_question_inputs(
    questions: Vec<QuestionSchema>,
) -> Result<Vec<UpsertQuestionInput>, errors::Error> {
    let questions = questions
        .into_iter()
        .map(into_upsert_question_input)
        .collect::<Result<Vec<_>, _>>()?;

    if questions.is_empty() {
        return Ok(questions);
    }

    QuestionSet::try_new(NonEmptyVec::try_new(
        questions
            .iter()
            .map(|question| question.question.clone())
            .collect(),
    )?)?;

    Ok(questions)
}

fn into_create_questions(
    questions: NonEmptyVec<QuestionSchema>,
) -> Result<NonEmptyVec<Question>, DomainError> {
    let questions = questions
        .into_inner()
        .into_iter()
        .enumerate()
        .map(|(position, question)| into_create_question(position as u16, question))
        .collect::<Result<Vec<_>, _>>()?;
    let questions = NonEmptyVec::try_new(questions).expect("create questions is non-empty");

    Ok(QuestionSet::try_new(questions)?.into_inner())
}

fn into_create_question(position: u16, question: QuestionSchema) -> Result<Question, DomainError> {
    let (question_type, definition, choices) = question.into_parts();

    match question_type {
        QuestionType::Text => Question::new_text(
            definition.template_key,
            position,
            definition.title,
            definition.description,
            definition.is_required,
        ),
        QuestionType::SingleChoice => Question::new_single_choice(
            definition.template_key,
            position,
            definition.title,
            definition.description,
            required_choices(into_domain_choices(choices))?,
            definition.is_required,
        ),
        QuestionType::MultipleChoice => Question::new_multiple_choice(
            definition.template_key,
            position,
            definition.title,
            definition.description,
            required_choices(into_domain_choices(choices))?,
            definition.is_required,
        ),
    }
}

fn into_upsert_question_input(
    question: QuestionSchema,
) -> Result<UpsertQuestionInput, DomainError> {
    let (question_type, definition, choices) = question.into_parts();
    let original_id = definition.id;
    let choices = into_domain_choices(choices);
    let question = match original_id {
        Some(question_id) => Question::from_raw_parts(
            question_id,
            definition.template_key,
            definition.position,
            definition.title,
            definition.description,
            question_type,
            choices,
            definition.is_required,
        )?,
        None => match question_type {
            QuestionType::Text => Question::new_text(
                definition.template_key,
                definition.position,
                definition.title,
                definition.description,
                definition.is_required,
            )?,
            QuestionType::SingleChoice => Question::new_single_choice(
                definition.template_key,
                definition.position,
                definition.title,
                definition.description,
                required_choices(choices)?,
                definition.is_required,
            )?,
            QuestionType::MultipleChoice => Question::new_multiple_choice(
                definition.template_key,
                definition.position,
                definition.title,
                definition.description,
                required_choices(choices)?,
                definition.is_required,
            )?,
        },
    };

    Ok(UpsertQuestionInput {
        original_id,
        question,
    })
}

fn into_domain_choices(choices: Option<Vec<ChoiceSchema>>) -> Option<NonEmptyVec<Choice>> {
    let choices = choices?;
    let choices = choices.into_iter().map(Into::into).collect::<Vec<_>>();
    (!choices.is_empty()).then(|| NonEmptyVec::try_new(choices).expect("non-empty choices"))
}

fn required_choices(
    choices: Option<NonEmptyVec<Choice>>,
) -> Result<NonEmptyVec<Choice>, DomainError> {
    choices.ok_or_else(|| DomainError::InvalidEntity {
        message: "choice question must have at least one choice".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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

        let result = into_upsert_question_input(question);

        assert!(matches!(result, Err(DomainError::InvalidEntity { .. })));
    }

    #[test]
    fn create_questions_assigns_contiguous_positions() {
        let questions: NonEmptyVec<QuestionSchema> = serde_json::from_value(json!([
            {
                "question_type": "Text",
                "template_key": "body",
                "position": 10,
                "title": "Body",
                "is_required": true
            },
            {
                "question_type": "Text",
                "template_key": "summary",
                "position": 20,
                "title": "Summary",
                "is_required": false
            }
        ]))
        .unwrap();

        let created = into_create_questions(questions).unwrap();

        assert_eq!(created[0].position(), 0);
        assert_eq!(created[1].position(), 1);
    }

    #[test]
    fn create_questions_rejects_duplicate_template_keys() {
        let questions: NonEmptyVec<QuestionSchema> = serde_json::from_value(json!([
            {
                "question_type": "Text",
                "template_key": "body",
                "position": 0,
                "title": "Body",
                "is_required": true
            },
            {
                "question_type": "Text",
                "template_key": "body",
                "position": 1,
                "title": "Summary",
                "is_required": false
            }
        ]))
        .unwrap();

        let result = into_create_questions(questions);

        assert!(matches!(result, Err(DomainError::InvalidEntity { .. })));
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

        let result = into_upsert_question_inputs(questions);

        assert!(matches!(
            result,
            Err(Error::Domain {
                source: DomainError::InvalidEntity { .. }
            })
        ));
    }
}
