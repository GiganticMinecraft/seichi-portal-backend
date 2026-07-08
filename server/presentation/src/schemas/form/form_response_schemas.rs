use chrono::{DateTime, Utc};
use domain::account::models::UserGroupId;
use domain::form::{
    answer::{AnswerEntry, AnswerLabel, FormAnswerContent},
    comment::CommentId,
    models::{
        AnswerSettings, DefaultAnswerTitle, FormDescription, FormId, FormLabel, FormMeta,
        FormSettings, FormTitle, Visibility,
    },
    question::{Choice, Question, QuestionType},
};
use domain::{account::models::AccountUser, auth::Actor};
use itertools::Itertools;
use serde::Serialize;
use types::non_empty_string::NonEmptyString;
use usecase::models::CommentWithAuthor;
use uuid::Uuid;

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct AnswerAcceptancePeriodSchema {
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub enum AnswerVisibility {
    #[serde(rename = "PUBLIC")]
    Public,
    #[serde(rename = "PRIVATE")]
    Private,
}

impl From<domain::form::models::AnswerVisibility> for AnswerVisibility {
    fn from(val: domain::form::models::AnswerVisibility) -> Self {
        match val {
            domain::form::models::AnswerVisibility::PUBLIC => AnswerVisibility::Public,
            domain::form::models::AnswerVisibility::PRIVATE => AnswerVisibility::Private,
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct AnswerSettingsSchema {
    #[schema(value_type = Option<String>)]
    pub default_answer_title: DefaultAnswerTitle,
    pub visibility: AnswerVisibility,
    pub acceptance_period: AnswerAcceptancePeriodSchema,
    #[schema(value_type = Vec<String>)]
    pub answer_group_ids: Vec<UserGroupId>,
}

impl AnswerSettingsSchema {
    pub fn from_answer_settings(answer_settings: &AnswerSettings) -> Self {
        Self {
            default_answer_title: answer_settings.default_answer_title().to_owned(),
            visibility: answer_settings.visibility().to_owned().into(),
            acceptance_period: AnswerAcceptancePeriodSchema {
                start_at: answer_settings.acceptance_period().start_at().to_owned(),
                end_at: answer_settings.acceptance_period().end_at().to_owned(),
            },
            answer_group_ids: answer_settings.answer_groups().as_slice().to_vec(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct FormSettingsSchema {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discord_webhook_url: Option<Option<String>>,
    #[schema(value_type = String)]
    pub visibility: Visibility,
    #[schema(value_type = Vec<String>)]
    pub allowed_group_ids: Vec<UserGroupId>,
    pub allow_temporary_answers: bool,
    pub answer_settings: AnswerSettingsSchema,
}

impl FormSettingsSchema {
    pub fn from_settings_and_answer_settings(
        actor: &Actor,
        settings: &FormSettings,
        answer_settings: &AnswerSettings,
    ) -> Self {
        FormSettingsSchema {
            discord_webhook_url: settings
                .discord_webhook_url(actor)
                .ok()
                .map(|url| url.to_owned().into_inner().map(NonEmptyString::into_inner)),
            visibility: settings.visibility().to_owned(),
            allowed_group_ids: settings.allowed_user_groups().as_slice().to_vec(),
            allow_temporary_answers: *answer_settings.allow_temporary_answers(),
            answer_settings: AnswerSettingsSchema::from_answer_settings(answer_settings),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct FormMetaSchema {
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl FormMetaSchema {
    pub fn from_meta_ref(meta: &FormMeta) -> Self {
        FormMetaSchema {
            created_at: meta.created_at,
            updated_at: meta.updated_at,
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct FormSchema {
    #[schema(value_type = String, format = "uuid")]
    pub id: FormId,
    #[schema(value_type = String)]
    pub title: FormTitle,
    #[schema(value_type = String)]
    pub description: FormDescription,
    pub settings: FormSettingsSchema,
    pub metadata: FormMetaSchema,
    pub questions: Vec<QuestionResponseSchema>,
    #[schema(value_type = Vec<FormLabelResponseSchema>)]
    pub labels: Vec<FormLabel>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct FormListPageResponse {
    pub items: Vec<FormSchema>,
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct ArchivedFormSchema {
    #[schema(value_type = String, format = "uuid")]
    pub id: FormId,
    #[schema(value_type = String)]
    pub title: FormTitle,
    #[schema(value_type = String)]
    pub description: FormDescription,
    pub settings: FormSettingsSchema,
    pub metadata: FormMetaSchema,
    pub archived_at: DateTime<Utc>,
    #[schema(value_type = serde_json::Value)]
    pub archived_by: AccountUser,
    pub questions: Vec<QuestionResponseSchema>,
    #[schema(value_type = Vec<FormLabelResponseSchema>)]
    pub labels: Vec<FormLabel>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct ArchivedFormListPageResponse {
    pub items: Vec<ArchivedFormSchema>,
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct QuestionDefinitionResponseSchema {
    #[schema(value_type = String, format = "uuid")]
    pub id: String,
    pub template_key: String,
    pub position: u16,
    pub title: String,
    pub description: Option<String>,
    pub is_required: bool,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct TextQuestionResponseSchema {
    #[serde(flatten)]
    pub definition: QuestionDefinitionResponseSchema,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct SelectQuestionResponseSchema {
    #[serde(flatten)]
    pub definition: QuestionDefinitionResponseSchema,
    pub choices: Vec<ChoiceResponseSchema>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
#[serde(tag = "question_type")]
pub enum QuestionResponseSchema {
    Text(TextQuestionResponseSchema),
    SingleChoice(SelectQuestionResponseSchema),
    MultipleChoice(SelectQuestionResponseSchema),
}

impl From<Question> for QuestionResponseSchema {
    fn from(val: Question) -> Self {
        let definition = QuestionDefinitionResponseSchema {
            id: val.id().into_inner().to_string(),
            template_key: val.template_key().to_owned().into_inner(),
            position: val.position(),
            title: val.title().to_owned().into_inner(),
            description: val.description().cloned().map(NonEmptyString::into_inner),
            is_required: val.is_required(),
        };

        match val.question_type() {
            QuestionType::Text => Self::Text(TextQuestionResponseSchema { definition }),
            QuestionType::SingleChoice => {
                let choices = val
                    .choices()
                    .cloned()
                    .map(|choices| choices.into_inner())
                    .unwrap_or_default()
                    .into_iter()
                    .map(Into::into)
                    .collect();
                Self::SingleChoice(SelectQuestionResponseSchema {
                    definition,
                    choices,
                })
            }
            QuestionType::MultipleChoice => {
                let choices = val
                    .choices()
                    .cloned()
                    .map(|choices| choices.into_inner())
                    .unwrap_or_default()
                    .into_iter()
                    .map(Into::into)
                    .collect();
                Self::MultipleChoice(SelectQuestionResponseSchema {
                    definition,
                    choices,
                })
            }
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct ChoiceResponseSchema {
    pub id: Option<i32>,
    pub position: u16,
    pub label: String,
}

impl From<Choice> for ChoiceResponseSchema {
    fn from(val: Choice) -> Self {
        Self {
            id: val.id.map(|id| id.into_inner()),
            position: val.position,
            label: val.label.into_inner(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct FormLabelResponseSchema {
    pub id: String,
    pub name: String,
}

impl From<FormLabel> for FormLabelResponseSchema {
    fn from(val: FormLabel) -> Self {
        FormLabelResponseSchema {
            id: val.id().to_owned().into_inner().to_string(),
            name: val.name().to_string(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct AnswerLabelResponseSchema {
    pub id: String,
    pub name: String,
}

impl From<AnswerLabel> for AnswerLabelResponseSchema {
    fn from(val: AnswerLabel) -> Self {
        AnswerLabelResponseSchema {
            id: val.id().to_owned().into_inner().to_string(),
            name: val.name().to_string(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub enum Role {
    #[serde(rename = "STANDARD_USER")]
    StandardUser,
    #[serde(rename = "ADMINISTRATOR")]
    Administrator,
}

impl From<domain::account::models::Role> for Role {
    fn from(val: domain::account::models::Role) -> Self {
        match val {
            domain::account::models::Role::StandardUser => Role::StandardUser,
            domain::account::models::Role::Administrator => Role::Administrator,
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct User {
    uuid: String,
    name: String,
    role: Role,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct TemporaryAnswerAuthor {
    id: String,
    name: String,
    contact_text: String,
}

impl From<domain::form::answer::TemporaryAnswerAuthor> for TemporaryAnswerAuthor {
    fn from(val: domain::form::answer::TemporaryAnswerAuthor) -> Self {
        TemporaryAnswerAuthor {
            id: val.id().to_string(),
            name: val.name().to_owned(),
            contact_text: val.contact_text().to_owned(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
#[serde(tag = "type")]
pub enum AnswerAuthor {
    #[serde(rename = "AUTHENTICATED_USER")]
    AuthenticatedUser { user: User },
    #[serde(rename = "TEMPORARY_USER")]
    Temporary {
        temporary_user: TemporaryAnswerAuthor,
    },
}

impl From<Actor> for AnswerAuthor {
    fn from(val: Actor) -> Self {
        match val {
            Actor::AccountUser(user) => AnswerAuthor::AuthenticatedUser { user: user.into() },
            Actor::TemporaryAnswerAuthor(temporary_user) => AnswerAuthor::Temporary {
                temporary_user: temporary_user.into(),
            },
            Actor::Anonymous => {
                unreachable!("Anonymous user cannot be an answer author")
            }
            Actor::System => unreachable!("System actor cannot be an answer author"),
        }
    }
}

impl From<AccountUser> for User {
    fn from(val: AccountUser) -> Self {
        User {
            uuid: val.id().to_string(),
            name: val.name().to_owned(),
            role: val.role().to_owned().into(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct AnswerContent {
    #[schema(value_type = String, format = "uuid")]
    question_id: String,
    answer: String,
}

impl AnswerContent {
    pub fn from_ref(val: &FormAnswerContent) -> Self {
        AnswerContent {
            question_id: val.question_id.into_inner().to_string(),
            answer: val.answer.to_string(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct AnswerComment {
    #[schema(value_type = String, format = "uuid")]
    id: CommentId,
    content: String,
    timestamp: DateTime<Utc>,
    commented_by: User,
}

impl From<CommentWithAuthor> for AnswerComment {
    fn from(val: CommentWithAuthor) -> Self {
        AnswerComment {
            id: val.comment.comment_id().to_owned(),
            content: val.comment.content().to_string(),
            timestamp: val.comment.timestamp().to_owned(),
            commented_by: val.commented_by.into(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct AnswerLabels {
    id: Uuid,
    name: String,
}

impl From<AnswerLabel> for AnswerLabels {
    fn from(val: AnswerLabel) -> Self {
        AnswerLabels {
            id: val.id().to_owned().into(),
            name: val.name().to_string(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct FormAnswer {
    id: Uuid,
    author: AnswerAuthor,
    form_id: Uuid,
    timestamp: DateTime<Utc>,
    title: Option<String>,
    answers: Vec<AnswerContent>,
    labels: Vec<AnswerLabels>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct AnswerListPageResponse {
    pub items: Vec<FormAnswer>,
    pub next_cursor: Option<String>,
}

impl FormAnswer {
    pub fn new(
        answer: AnswerEntry,
        form_id: FormId,
        author: Actor,
        labels: Vec<AnswerLabel>,
    ) -> Self {
        FormAnswer {
            id: answer.id().to_owned().into(),
            author: author.into(),
            form_id: form_id.into_inner(),
            timestamp: answer.timestamp().to_owned(),
            title: answer
                .title()
                .to_owned()
                .into_inner()
                .map(|title| title.to_string()),
            answers: answer
                .contents()
                .iter()
                .map(AnswerContent::from_ref)
                .collect_vec(),
            labels: labels.into_iter().map(Into::into).collect_vec(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct MessageContentSchema {
    pub id: Uuid,
    pub body: String,
    pub sender: SenderSchema,
    pub timestamp: DateTime<Utc>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct SenderSchema {
    pub uuid: String,
    pub name: String,
    pub role: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::form::question::{Choice, Question};
    use types::non_empty_vec::NonEmptyVec;

    #[test]
    fn question_response_schema_serializes_text_variant_without_choices() {
        let question = Question::new_text(
            "body".to_string().try_into().unwrap(),
            0,
            "Body".to_string().try_into().unwrap(),
            Some("desc".to_string().try_into().unwrap()),
            true,
        )
        .unwrap();

        let schema = QuestionResponseSchema::from(question);
        let serialized = serde_json::to_value(schema).unwrap();

        assert_eq!(serialized["question_type"], "Text");
        assert_eq!(serialized["template_key"], "body");
        assert!(serialized.get("choices").is_none());
        assert_eq!(serialized["is_required"], true);
    }

    #[test]
    fn question_response_schema_preserves_api_shape_for_choice_question() {
        let question = Question::new_single_choice(
            "role".to_string().try_into().unwrap(),
            0,
            "Role".to_string().try_into().unwrap(),
            Some("desc".to_string().try_into().unwrap()),
            NonEmptyVec::try_new(vec![
                Choice::new(Some(10.into()), 0, "Admin".to_string().try_into().unwrap()),
                Choice::new(Some(11.into()), 1, "User".to_string().try_into().unwrap()),
            ])
            .unwrap(),
            true,
        )
        .unwrap();

        let schema = QuestionResponseSchema::from(question);
        let serialized = serde_json::to_value(schema).unwrap();

        assert_eq!(serialized["question_type"], "SingleChoice");
        assert_eq!(serialized["choices"].as_array().unwrap().len(), 2);
        assert_eq!(serialized["choices"][0]["label"], "Admin");
        assert_eq!(serialized["is_required"], true);
    }
}
