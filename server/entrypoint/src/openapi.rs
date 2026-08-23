use presentation::handlers::{
    global_discord_webhook_handler, notification_handler, search_handler, user_handler,
};
use resource::repository::RealInfrastructureRepository;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Seichi Portal API",
        version = "1.0.0",
        description = "## レートリミット\n\nAPI には、クライアント単位またはアカウント単位のレートリミットがあります。制限を超えた場合は `429 Too Many Requests` を返し、`Retry-After`、`RateLimit-Limit`、`RateLimit-Remaining`、`RateLimit-Reset` ヘッダーで再試行できる時刻を示します。\n\n- 未認証の GET: クライアント IP ごとに 1 分あたり 60 回\n- 一時回答の POST: クライアント IP ごとに 1 時間あたり 30 回、フォームごとに 1 時間あたり 10 回、クライアント IP ごとに 10 分あたり 5 回\n- セッション作成の POST: クライアント IP ごとに 1 時間あたり 10 回\n- 認証済みの GET: アカウントごとに 1 分あたり 600 回\n- 認証済みの書き込み: アカウントごとに 1 分あたり 120 回\n\n認証済みのリクエストはアカウント ID、未認証のリクエストはクライアント IP を基準に制限します。フロントエンドのプロキシがクライアント IP を転送する場合は、`X-Seichi-Proxy-Secret` が一致したときだけ `X-Seichi-Client-IP` を信頼します。"
    ),
    components(schemas(
        presentation::schemas::error_response::ErrorResponse,
        presentation::schemas::error_response::ErrorRestriction,
        presentation::schemas::user::UserInfoResponse,
        presentation::schemas::user::UserListPageResponse,
        presentation::schemas::user::UserSchema,
        presentation::schemas::user::UserGroupRequest,
        presentation::schemas::user::UserGroupSchema,
        presentation::schemas::user::FormSubmissionRestrictionRequest,
        presentation::schemas::user::FormSubmissionRestrictionResponse,
        presentation::schemas::user::FormSubmissionRestrictionHistoryResponse,
        presentation::schemas::user::MinecraftPunishmentResponse,
        presentation::schemas::form::form_response_schemas::AnswerComment,
        presentation::schemas::form::form_response_schemas::AnswerContent,
        presentation::schemas::form::form_response_schemas::AnswerLabels,
        presentation::schemas::form::form_response_schemas::AnswerAuthor,
        presentation::schemas::form::form_response_schemas::AnswerLabelResponseSchema,
        presentation::schemas::form::form_response_schemas::AnswerListPageResponse,
        presentation::schemas::form::form_response_schemas::AnswerStatusHistoryPageResponse,
        presentation::schemas::form::form_response_schemas::AnswerTitleHistoryPageResponse,
        presentation::schemas::form::form_response_schemas::AnswerSettingsSchema,
        presentation::schemas::form::form_response_schemas::AnswerVisibility,
        presentation::schemas::form::form_response_schemas::ArchivedFormListPageResponse,
        presentation::schemas::form::form_response_schemas::ArchivedFormSchema,
        presentation::schemas::form::form_response_schemas::FormAnswer,
        presentation::schemas::form::form_response_schemas::FormLabelResponseSchema,
        presentation::schemas::form::form_response_schemas::FormListPageResponse,
        presentation::schemas::form::form_response_schemas::FormMetaSchema,
        presentation::schemas::form::form_response_schemas::FormSchema,
        presentation::schemas::form::form_response_schemas::FormSettingsResponseSchema,
        presentation::schemas::form::form_response_schemas::TemporaryAnswerAuthor,
        presentation::schemas::form::form_response_schemas::MessageContentSchema,
        presentation::schemas::form::form_response_schemas::ChoiceResponseSchema,
        presentation::schemas::form::form_response_schemas::QuestionDefinitionResponseSchema,
        presentation::schemas::form::form_response_schemas::QuestionResponseSchema,
        presentation::schemas::form::form_response_schemas::SelectQuestionResponseSchema,
        presentation::schemas::form::form_response_schemas::TextQuestionResponseSchema,
        presentation::schemas::form::form_response_schemas::RelatedAnswerResponse,
        presentation::schemas::form::form_request_schemas::ChoiceSchema,
        presentation::schemas::form::form_request_schemas::QuestionDefinitionSchema,
        presentation::schemas::form::form_request_schemas::QuestionSchema,
        presentation::schemas::form::form_request_schemas::SelectQuestionSchema,
        presentation::schemas::form::form_request_schemas::TextQuestionSchema,
        presentation::schemas::form::form_request_schemas::TemporaryAnswerCreateSchema,
        presentation::schemas::form::form_request_schemas::RelatedAnswerRequest,
        presentation::schemas::form::form_request_schemas::TemporaryUserCreateSchema,
        presentation::schemas::form::form_response_schemas::AnswerAcceptancePeriodSchema,
        presentation::schemas::form::form_response_schemas::Role,
        presentation::schemas::form::form_response_schemas::SenderSchema,
        presentation::schemas::form::form_response_schemas::User,
        presentation::schemas::notification::notification_response_schemas::NotificationPageResponse,
        presentation::schemas::notification::notification_response_schemas::NotificationResponse,
        presentation::schemas::notification::notification_response_schemas::NotificationSettingsResponse,
        presentation::schemas::search_schemas::SearchCommentSchema,
        presentation::schemas::search_schemas::CrossSearchResult,
        presentation::schemas::search_schemas::UserSearchResult,
        presentation::schemas::search_schemas::AnswerSearchResult,
    )),
    modifiers(&SecurityAddon),
    tags(
        (name = "Forms"),
        (name = "Archived Forms"),
        (name = "Answers"),
        (name = "Comments"),
        (name = "Labels"),
        (name = "Messages"),
        (name = "Users"),
        (name = "User Groups"),
        (name = "Search"),
        (name = "Notifications"),
        (name = "Settings"),
        (name = "Session"),
        (name = "Health"),
    )
)]
struct ApiMetadata;

#[derive(OpenApi)]
#[openapi(paths(presentation::handlers::form::message_handler::post_message_handler,))]
struct ManuallyRegisteredApiDoc;

struct SecurityAddon;

impl Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "bearer",
                SecurityScheme::Http(HttpBuilder::new().scheme(HttpAuthScheme::Bearer).build()),
            );
        }
    }
}

pub fn public_api_router() -> OpenApiRouter<RealInfrastructureRepository> {
    use presentation::handlers::form::answer_handler;

    OpenApiRouter::new()
        .routes(routes!(answer_handler::post_temporary_answer_handler))
        .routes(routes!(user_handler::start_session))
}

pub fn authenticated_session_api_router() -> OpenApiRouter<RealInfrastructureRepository> {
    use presentation::handlers::user_handler;

    OpenApiRouter::new().routes(routes!(user_handler::end_session))
}

pub fn optional_auth_api_router() -> OpenApiRouter<RealInfrastructureRepository> {
    use presentation::handlers::form::form_handler;

    OpenApiRouter::new()
        .routes(routes!(form_handler::form_list_handler))
        .routes(routes!(form_handler::get_form_handler))
}

pub fn authenticated_api_router() -> OpenApiRouter<RealInfrastructureRepository> {
    use presentation::handlers::form::{
        answer_handler, answer_label_handler, answer_relation_handler, comment_handler,
        form_handler, form_label_handler, message_handler,
    };

    OpenApiRouter::new()
        .routes(routes!(
            global_discord_webhook_handler::get_global_discord_webhook,
            global_discord_webhook_handler::update_global_discord_webhook
        ))
        .routes(routes!(form_handler::create_form_handler))
        .routes(routes!(form_handler::update_form_handler))
        .routes(routes!(form_handler::archive_form_handler))
        .routes(routes!(form_handler::archived_form_list_handler))
        .routes(routes!(form_handler::get_archived_form_handler))
        .routes(routes!(form_handler::restore_archived_form_handler))
        .routes(routes!(
            answer_handler::get_answer_by_form_id_handler,
            answer_handler::post_answer_handler
        ))
        .routes(routes!(
            answer_relation_handler::get_related_answers_handler,
            answer_relation_handler::add_related_answer_handler,
            answer_relation_handler::remove_related_answer_handler
        ))
        .routes(routes!(answer_handler::get_all_answers))
        .routes(routes!(
            answer_label_handler::get_labels_for_answers,
            answer_label_handler::create_label_for_answers
        ))
        .routes(routes!(
            answer_label_handler::delete_label_for_answers,
            answer_label_handler::edit_label_for_answers
        ))
        .routes(routes!(
            form_label_handler::get_labels_for_forms,
            form_label_handler::create_label_for_forms
        ))
        .routes(routes!(
            form_label_handler::delete_label_for_forms,
            form_label_handler::edit_label_for_forms
        ))
        .routes(routes!(
            answer_handler::get_answer_handler,
            answer_handler::update_answer_handler
        ))
        .routes(routes!(answer_handler::get_answer_status_history_handler))
        .routes(routes!(answer_handler::get_answer_title_history_handler))
        .routes(routes!(answer_label_handler::replace_answer_labels))
        .routes(routes!(
            comment_handler::get_form_comment,
            comment_handler::post_form_comment
        ))
        .routes(routes!(comment_handler::get_comment_history))
        .routes(routes!(
            comment_handler::update_form_comment,
            comment_handler::delete_form_comment_handler
        ))
        .routes(routes!(
            user_handler::get_user_info,
            user_handler::patch_user_role
        ))
        .routes(routes!(user_handler::get_my_user_info))
        .routes(routes!(user_handler::user_list))
        .routes(routes!(
            user_handler::create_user_group,
            user_handler::user_group_list
        ))
        .routes(routes!(
            user_handler::update_user_group,
            user_handler::delete_user_group
        ))
        .routes(routes!(user_handler::user_group_user_list))
        .routes(routes!(
            user_handler::add_user_to_group,
            user_handler::remove_user_from_group
        ))
        .routes(routes!(
            user_handler::get_form_submission_restriction,
            user_handler::put_form_submission_restriction,
            user_handler::delete_form_submission_restriction
        ))
        .routes(routes!(
            user_handler::get_form_submission_restriction_history
        ))
        .routes(routes!(user_handler::get_minecraft_punishments))
        .routes(routes!(search_handler::cross_search))
        .routes(routes!(search_handler::search_users))
        .routes(routes!(search_handler::search_answers))
        .routes(routes!(message_handler::get_messages_handler))
        .routes(routes!(message_handler::get_message_history))
        .routes(routes!(
            message_handler::update_message_handler,
            message_handler::delete_message_handler
        ))
        .routes(routes!(notification_handler::get_notification_settings))
        .routes(routes!(notification_handler::get_notifications))
        .routes(routes!(notification_handler::mark_notification_as_read))
        .routes(routes!(
            notification_handler::mark_all_notifications_as_read
        ))
        .routes(routes!(
            notification_handler::get_my_notification_settings,
            notification_handler::update_notification_settings
        ))
        .routes(routes!(
            user_handler::link_discord,
            user_handler::unlink_discord
        ))
}

pub fn versioned_api_router() -> OpenApiRouter<RealInfrastructureRepository> {
    let combined = OpenApiRouter::with_openapi(ManuallyRegisteredApiDoc::openapi())
        .merge(public_api_router())
        .merge(authenticated_session_api_router())
        .merge(optional_auth_api_router())
        .merge(authenticated_api_router());
    OpenApiRouter::with_openapi(ApiMetadata::openapi()).nest("/api/v1", combined)
}

pub fn openapi() -> utoipa::openapi::OpenApi {
    versioned_api_router().into_openapi()
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::openapi;

    fn collect_refs(value: &Value, refs: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                for (key, value) in map {
                    match (key.as_str(), value) {
                        ("$ref", Value::String(reference)) => refs.push(reference.clone()),
                        _ => collect_refs(value, refs),
                    }
                }
            }
            Value::Array(values) => values.iter().for_each(|value| collect_refs(value, refs)),
            _ => {}
        }
    }

    #[test]
    fn openapi_document_has_paths() {
        let document = serde_json::to_value(openapi()).expect("OpenAPI document must serialize");

        let paths = document
            .pointer("/paths")
            .and_then(Value::as_object)
            .expect("OpenAPI document must have paths");

        assert!(!paths.is_empty(), "OpenAPI document must not be empty");
    }

    #[test]
    fn all_refs_in_openapi_document_are_resolvable() {
        let document = serde_json::to_value(openapi()).expect("OpenAPI document must serialize");

        let mut refs = Vec::new();
        collect_refs(&document, &mut refs);

        let unresolved = refs
            .iter()
            .filter(|reference| {
                reference
                    .strip_prefix('#')
                    .is_none_or(|pointer| document.pointer(pointer).is_none())
            })
            .collect::<Vec<_>>();

        assert!(
            unresolved.is_empty(),
            "OpenAPI ドキュメントに解決できない $ref があります。\
             ハンドラの utoipa::path で参照しているスキーマが \
             openapi.rs の components(schemas(...)) に登録されているか確認してください: {unresolved:?}"
        );
    }
}
