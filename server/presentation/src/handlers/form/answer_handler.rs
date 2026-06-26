use async_trait::async_trait;
use axum::extract::rejection::{JsonRejection, PathRejection};
use axum::response::Response;
use axum::{
    Extension, Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use domain::form::answer::{FormAnswerContent, FormAnswerContentId};
use domain::{
    account::models::AccountUser,
    form::answer::TemporaryAnswerAuthor,
    form::{answer::AnswerId, models::FormId},
    repository::Repositories,
};
use errors::ErrorExtra;
use itertools::Itertools;
use resource::{
    outgoing::discord_webhook_sender::{
        DiscordWebhookField, DiscordWebhookMessage, DiscordWebhookSender,
    },
    repository::{RealInfrastructureRepository, Repository},
};
use serde_json::json;
use tracing::warn;
use usecase::forms::{
    answer::AnswerUseCase,
    discord_answer_webhook::{DiscordAnswerWebhookNotification, DiscordAnswerWebhookNotifier},
};

use crate::schemas::error_responses::*;
use crate::{
    handlers::error_handler::handle_error,
    schemas::form::{
        form_request_schemas::{
            AnswerCreateSchema, AnswerUpdateSchema, TemporaryAnswerCreateSchema,
        },
        form_response_schemas::FormAnswer,
    },
};

type ResourceRepository = Repository<resource::database::connection::ConnectionPool>;
type ResourceAnswerUseCase<'a> = AnswerUseCase<
    'a,
    ResourceRepository,
    ResourceRepository,
    ResourceRepository,
    ResourceRepository,
>;

fn build_answer_use_case<'a>(
    repository: &'a RealInfrastructureRepository,
    discord_answer_webhook_notifier: Option<&'a dyn DiscordAnswerWebhookNotifier>,
) -> ResourceAnswerUseCase<'a> {
    AnswerUseCase {
        active_form_repository: repository.active_form_repository(),
        answer_label_repository: repository.answer_label_repository(),
        user_repository: repository.user_repository(),
        answer_entry_repository: repository.answer_entry_repository(),
        discord_answer_webhook_notifier,
    }
}

struct ResourceDiscordAnswerWebhookNotifier {
    sender: DiscordWebhookSender,
}

impl ResourceDiscordAnswerWebhookNotifier {
    fn new(sender: DiscordWebhookSender) -> Self {
        Self { sender }
    }
}

#[async_trait]
impl DiscordAnswerWebhookNotifier for ResourceDiscordAnswerWebhookNotifier {
    async fn notify_answer_posted(&self, notification: DiscordAnswerWebhookNotification) {
        let sender = self.sender.clone();
        tokio::spawn(async move {
            let form_id = notification.form_id.clone();
            let answer_id = notification.answer_id.clone();
            let attempts = DiscordWebhookSender::retry_policy().max_attempts();
            let message = DiscordWebhookMessage {
                discord_webhook_url: notification.discord_webhook_url,
                title: "回答が送信されました".to_string(),
                link_url: notification.answer_url,
                fields: notification
                    .fields
                    .into_iter()
                    .map(|field| DiscordWebhookField::new(field.name, field.value, false))
                    .collect(),
            };

            if let Err(error) = sender.send_with_retry(message).await {
                warn!(
                    form_id,
                    answer_id,
                    attempts,
                    error = %error,
                    "failed to send Discord answer webhook after retries"
                );
            }
        });
    }
}

#[derive(utoipa::IntoResponses)]
pub enum GetAllAnswersResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(Vec<FormAnswer>),
}

impl IntoResponse for GetAllAnswersResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(json!(body))).into_response(),
        }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum GetAnswerResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(FormAnswer),
}

impl IntoResponse for GetAnswerResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(json!(body))).into_response(),
        }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum GetAnswersByFormResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(Vec<FormAnswer>),
}

impl IntoResponse for GetAnswersByFormResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

#[derive(utoipa::IntoResponses)]
pub enum UpdateAnswerResponse {
    #[response(status = 200, description = "The request has succeeded.")]
    Ok(FormAnswer),
}

impl IntoResponse for UpdateAnswerResponse {
    fn into_response(self) -> Response {
        match self {
            Self::Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        }
    }
}

#[utoipa::path(
    get,
    path = "/forms/answers",
    summary = "すべての回答をフォームを横断して取得",
    responses(
        GetAllAnswersResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Answers"
)]
pub async fn get_all_answers(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
) -> Result<GetAllAnswersResponse, Response> {
    let form_answer_use_case = build_answer_use_case(&repository, None);

    let answers = form_answer_use_case
        .get_all_answers(&user)
        .await
        .map_err(handle_error)?;

    Ok(GetAllAnswersResponse::Ok(
        answers
            .into_iter()
            .map(|answer_details| {
                FormAnswer::new(
                    answer_details.form_answer,
                    answer_details.form_id,
                    answer_details.author,
                    answer_details.labels,
                )
            })
            .collect_vec(),
    ))
}

#[utoipa::path(
    get,
    path = "/forms/{form_id}/answers/{answer_id}",
    summary = "回答の取得",
    params(
        ("form_id" = String, Path, description = "Form ID"),
        ("answer_id" = String, Path, description = "Answer ID"),
    ),
    responses(
        GetAnswerResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Answers"
)]
pub async fn get_answer_handler(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId)>, PathRejection>,
) -> Result<GetAnswerResponse, Response> {
    let form_answer_use_case = build_answer_use_case(&repository, None);

    let Path((form_id, answer_id)) = path.map_err_to_error().map_err(handle_error)?;

    let answer_details = form_answer_use_case
        .get_answers(form_id, answer_id, &user)
        .await
        .map_err(handle_error)?;

    Ok(GetAnswerResponse::Ok(FormAnswer::new(
        answer_details.form_answer,
        answer_details.form_id,
        answer_details.author,
        answer_details.labels,
    )))
}

#[utoipa::path(
    get,
    path = "/forms/{id}/answers",
    summary = "回答の一覧取得",
    params(
        ("id" = String, Path, description = "Form ID"),
    ),
    responses(
        GetAnswersByFormResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Answers"
)]
pub async fn get_answer_by_form_id_handler(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormId>, PathRejection>,
) -> Result<GetAnswersByFormResponse, Response> {
    let form_answer_use_case = build_answer_use_case(&repository, None);

    let Path(form_id) = path.map_err_to_error().map_err(handle_error)?;

    let answers = form_answer_use_case
        .get_answers_by_form_id(form_id, &user)
        .await
        .map_err(handle_error)?;

    Ok(GetAnswersByFormResponse::Ok(
        answers
            .into_iter()
            .map(|answer_details| {
                FormAnswer::new(
                    answer_details.form_answer,
                    answer_details.form_id,
                    answer_details.author,
                    answer_details.labels,
                )
            })
            .collect_vec(),
    ))
}

#[utoipa::path(
    post,
    path = "/forms/{id}/answers",
    summary = "回答の作成",
    params(
        ("id" = String, Path, description = "Form ID"),
    ),
    request_body = AnswerCreateSchema,
    responses(
        (status = 200, description = "The request has succeeded."),
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Answers"
)]
pub async fn post_answer_handler(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormId>, PathRejection>,
    json: Result<Json<AnswerCreateSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let discord_answer_webhook_notifier =
        ResourceDiscordAnswerWebhookNotifier::new(DiscordWebhookSender::new());
    let form_answer_use_case =
        build_answer_use_case(&repository, Some(&discord_answer_webhook_notifier));

    let Path(form_id) = path.map_err_to_error().map_err(handle_error)?;
    let Json(schema) = json.map_err_to_error().map_err(handle_error)?;

    let answer_contents = schema
        .contents
        .into_iter()
        .map(|schema| FormAnswerContent {
            id: FormAnswerContentId::new(),
            question_id: schema.question_id,
            answer: schema.answer,
        })
        .collect_vec();

    form_answer_use_case
        .post_answers(user, form_id, answer_contents)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::OK.into_response())
}

#[utoipa::path(
    post,
    path = "/forms/{id}/temporary-answers",
    summary = "未ログイン回答の作成",
    params(
        ("id" = String, Path, description = "Form ID"),
    ),
    request_body = TemporaryAnswerCreateSchema,
    responses(
        (status = 200, description = "The request has succeeded."),
        BadRequest,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    tag = "Answers"
)]
pub async fn post_temporary_answer_handler(
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<FormId>, PathRejection>,
    json: Result<Json<TemporaryAnswerCreateSchema>, JsonRejection>,
) -> Result<impl IntoResponse, Response> {
    let discord_answer_webhook_notifier =
        ResourceDiscordAnswerWebhookNotifier::new(DiscordWebhookSender::new());
    let form_answer_use_case =
        build_answer_use_case(&repository, Some(&discord_answer_webhook_notifier));

    let Path(form_id) = path.map_err_to_error().map_err(handle_error)?;
    let Json(schema) = json.map_err_to_error().map_err(handle_error)?;

    let temporary_user = TemporaryAnswerAuthor::new(
        schema.temporary_user.name.into_inner(),
        schema.temporary_user.contact_text.into_inner(),
    );
    let answer_contents = schema
        .contents
        .into_iter()
        .map(|schema| FormAnswerContent {
            id: FormAnswerContentId::new(),
            question_id: schema.question_id,
            answer: schema.answer,
        })
        .collect_vec();

    form_answer_use_case
        .post_temporary_answers(temporary_user, form_id, answer_contents)
        .await
        .map_err(handle_error)?;

    Ok(StatusCode::OK.into_response())
}

#[utoipa::path(
    patch,
    path = "/forms/{form_id}/answers/{answer_id}",
    summary = "回答の更新",
    params(
        ("form_id" = String, Path, description = "Form ID"),
        ("answer_id" = String, Path, description = "Answer ID"),
    ),
    request_body = AnswerUpdateSchema,
    responses(
        UpdateAnswerResponse,
        BadRequest,
        Unauthorized,
        Forbidden,
        NotFound,
        UnprocessableEntity,
        InternalServerError,
    ),
    security(("bearer" = [])),
    tag = "Answers"
)]
pub async fn update_answer_handler(
    Extension(user): Extension<AccountUser>,
    State(repository): State<RealInfrastructureRepository>,
    path: Result<Path<(FormId, AnswerId)>, PathRejection>,
    json: Result<Json<AnswerUpdateSchema>, JsonRejection>,
) -> Result<UpdateAnswerResponse, Response> {
    let form_answer_use_case = build_answer_use_case(&repository, None);

    let Path((form_id, answer_id)) = path.map_err_to_error().map_err(handle_error)?;
    let Json(schema) = json.map_err_to_error().map_err(handle_error)?;

    let answer_details = form_answer_use_case
        .update_answer_meta(form_id, answer_id, &user, schema.title)
        .await
        .map_err(handle_error)?;

    Ok(UpdateAnswerResponse::Ok(FormAnswer::new(
        answer_details.form_answer,
        answer_details.form_id,
        answer_details.author,
        answer_details.labels,
    )))
}
