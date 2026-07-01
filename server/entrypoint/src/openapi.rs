use presentation::handlers::{notification_handler, search_handler, user_handler};
use resource::repository::RealInfrastructureRepository;
use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};
use utoipa::{Modify, OpenApi};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

#[derive(OpenApi)]
#[openapi(
    info(title = "Seichi Portal API", version = "1.0.0"),
    components(schemas(
        presentation::schemas::error_response::ErrorResponse,
        presentation::schemas::error_response::ErrorRestriction,
        presentation::schemas::user::UserInfoResponse,
        presentation::schemas::user::UserListPageResponse,
        presentation::schemas::user::UserSchema,
        presentation::schemas::user::UserGroupRequest,
        presentation::schemas::user::UserGroupSchema,
        presentation::schemas::user::AnswerSubmitterRestrictionRequest,
        presentation::schemas::user::AnswerSubmitterRestrictionResponse,
        presentation::schemas::user::AnswerSubmitterRestrictionHistoryResponse,
        presentation::schemas::form::form_response_schemas::AnswerComment,
        presentation::schemas::form::form_response_schemas::AnswerContent,
        presentation::schemas::form::form_response_schemas::AnswerLabels,
        presentation::schemas::form::form_response_schemas::AnswerAuthor,
        presentation::schemas::form::form_response_schemas::AnswerLabelResponseSchema,
        presentation::schemas::form::form_response_schemas::AnswerListPageResponse,
        presentation::schemas::form::form_response_schemas::AnswerSettingsSchema,
        presentation::schemas::form::form_response_schemas::AnswerVisibility,
        presentation::schemas::form::form_response_schemas::ArchivedFormListPageResponse,
        presentation::schemas::form::form_response_schemas::ArchivedFormSchema,
        presentation::schemas::form::form_response_schemas::FormAnswer,
        presentation::schemas::form::form_response_schemas::FormLabelResponseSchema,
        presentation::schemas::form::form_response_schemas::FormListPageResponse,
        presentation::schemas::form::form_response_schemas::FormMetaSchema,
        presentation::schemas::form::form_response_schemas::FormSchema,
        presentation::schemas::form::form_response_schemas::FormSettingsSchema,
        presentation::schemas::form::form_response_schemas::TemporaryAnswerAuthor,
        presentation::schemas::form::form_response_schemas::MessageContentSchema,
        presentation::schemas::form::form_response_schemas::ChoiceResponseSchema,
        presentation::schemas::form::form_response_schemas::QuestionDefinitionResponseSchema,
        presentation::schemas::form::form_response_schemas::QuestionResponseSchema,
        presentation::schemas::form::form_response_schemas::SelectQuestionResponseSchema,
        presentation::schemas::form::form_response_schemas::TextQuestionResponseSchema,
        presentation::schemas::form::form_request_schemas::ChoiceSchema,
        presentation::schemas::form::form_request_schemas::QuestionDefinitionSchema,
        presentation::schemas::form::form_request_schemas::QuestionSchema,
        presentation::schemas::form::form_request_schemas::SelectQuestionSchema,
        presentation::schemas::form::form_request_schemas::TextQuestionSchema,
        presentation::schemas::form::form_request_schemas::TemporaryAnswerCreateSchema,
        presentation::schemas::form::form_request_schemas::TemporaryUserCreateSchema,
        presentation::schemas::form::form_response_schemas::AnswerAcceptancePeriodSchema,
        presentation::schemas::form::form_response_schemas::Role,
        presentation::schemas::form::form_response_schemas::SenderSchema,
        presentation::schemas::form::form_response_schemas::User,
        presentation::schemas::notification::notification_response_schemas::NotificationSettingsResponse,
        presentation::schemas::search_schemas::CommentSchema,
        presentation::schemas::search_schemas::CrossSearchResult,
        presentation::schemas::search_schemas::UserSearchResult,
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
        .routes(routes!(
            user_handler::start_session,
            user_handler::end_session
        ))
}

pub fn optional_auth_api_router() -> OpenApiRouter<RealInfrastructureRepository> {
    use presentation::handlers::form::form_handler;

    OpenApiRouter::new()
        .routes(routes!(form_handler::form_list_handler))
        .routes(routes!(form_handler::get_form_handler))
}

pub fn authenticated_api_router() -> OpenApiRouter<RealInfrastructureRepository> {
    use presentation::handlers::form::{
        answer_handler, answer_label_handler, comment_handler, form_handler, form_label_handler,
        message_handler,
    };

    OpenApiRouter::new()
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
        .routes(routes!(answer_label_handler::replace_answer_labels))
        .routes(routes!(
            comment_handler::get_form_comment,
            comment_handler::post_form_comment
        ))
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
            user_handler::get_answer_submitter_restriction,
            user_handler::put_answer_submitter_restriction,
            user_handler::delete_answer_submitter_restriction
        ))
        .routes(routes!(
            user_handler::get_answer_submitter_restriction_history
        ))
        .routes(routes!(search_handler::cross_search))
        .routes(routes!(search_handler::search_users))
        .routes(routes!(message_handler::get_messages_handler))
        .routes(routes!(
            message_handler::update_message_handler,
            message_handler::delete_message_handler
        ))
        .routes(routes!(notification_handler::get_notification_settings))
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
        .merge(optional_auth_api_router())
        .merge(authenticated_api_router());
    OpenApiRouter::with_openapi(ApiMetadata::openapi()).nest("/api/v1", combined)
}

pub fn openapi() -> utoipa::openapi::OpenApi {
    versioned_api_router().into_openapi()
}
