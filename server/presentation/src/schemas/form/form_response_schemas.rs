use chrono::{DateTime, Utc};
use domain::account::models::AccountUser;
use domain::account::models::{UserGroupId, UserSnapshot};
use domain::form::{
    answer::{
        AnswerLabel, AnswerPublication as DomainAnswerPublication, AnswerReference,
        AnswerStatus as DomainAnswerStatus, AnswerStatusHistoryEntry, AnswerTitleHistoryEntry,
        FormAnswerContent, RedmineUserSnapshot,
    },
    comment::{CommentHistoryAction, CommentHistoryEntry, CommentId},
    message::{MessageHistoryAction, MessageHistoryEntry},
    models::{
        ActiveForm, AnswerResponseVisibility, AnswerSettings, DefaultAnswerTitle, FormDescription,
        FormId, FormLabel, FormMeta, FormSettings, FormTitle, Visibility,
    },
    question::{Choice, Question, QuestionType},
};
use itertools::Itertools;
use serde::{Serialize, Serializer, ser::SerializeStruct};
use types::non_empty_string::NonEmptyString;
use usecase::models::{
    CommentAuthor, CommentWithAuthor, PublishedAnswerAuthor, PublishedAnswerEntry,
};
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

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub enum AnswerPublication {
    #[serde(rename = "PUBLIC")]
    Public,
    #[serde(rename = "PRIVATE")]
    Private,
}

impl From<DomainAnswerPublication> for AnswerPublication {
    fn from(val: DomainAnswerPublication) -> Self {
        match val {
            DomainAnswerPublication::PUBLIC => AnswerPublication::Public,
            DomainAnswerPublication::PRIVATE => AnswerPublication::Private,
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema, Copy, Clone)]
pub enum AnswerStatus {
    #[serde(rename = "UNADDRESSED")]
    Unaddressed,
    #[serde(rename = "IN_PROGRESS")]
    InProgress,
    #[serde(rename = "COMPLETED")]
    Completed,
}

impl From<DomainAnswerStatus> for AnswerStatus {
    fn from(value: DomainAnswerStatus) -> Self {
        match value {
            DomainAnswerStatus::UNADDRESSED => Self::Unaddressed,
            DomainAnswerStatus::IN_PROGRESS => Self::InProgress,
            DomainAnswerStatus::COMPLETED => Self::Completed,
        }
    }
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
    pub hide_author: bool,
    #[schema(value_type = Option<String>)]
    pub default_answer_title: DefaultAnswerTitle,
    pub visibility: AnswerVisibility,
    #[schema(value_type = String)]
    pub answer_response_visibility: AnswerResponseVisibility,
    pub acceptance_period: AnswerAcceptancePeriodSchema,
    #[schema(value_type = Vec<String>)]
    pub answer_group_ids: Vec<UserGroupId>,
}

impl AnswerSettingsSchema {
    pub fn from_answer_settings(answer_settings: &AnswerSettings) -> Self {
        Self {
            hide_author: answer_settings.author_publication_policy().hides_author(),
            default_answer_title: answer_settings.default_answer_title().to_owned(),
            visibility: answer_settings.visibility().to_owned().into(),
            answer_response_visibility: *answer_settings.answer_response_visibility(),
            acceptance_period: AnswerAcceptancePeriodSchema {
                start_at: answer_settings.acceptance_period().start_at().to_owned(),
                end_at: answer_settings.acceptance_period().end_at().to_owned(),
            },
            answer_group_ids: answer_settings.answer_group_ids().to_vec(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct FormSettingsResponseSchema {
    pub discord_webhook_enabled: bool,
    #[schema(value_type = String)]
    pub visibility: Visibility,
    #[schema(value_type = Vec<String>)]
    pub allowed_group_ids: Vec<UserGroupId>,
    pub allow_temporary_answers: bool,
    pub answer_settings: AnswerSettingsSchema,
}

impl FormSettingsResponseSchema {
    pub fn from_settings_and_answer_settings(
        settings: &FormSettings,
        answer_settings: &AnswerSettings,
    ) -> Self {
        FormSettingsResponseSchema {
            discord_webhook_enabled: settings.discord_webhook_enabled(),
            visibility: settings.visibility().to_owned(),
            allowed_group_ids: settings.allowed_user_groups().as_slice().to_vec(),
            allow_temporary_answers: answer_settings.allow_temporary_answers(),
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
    pub settings: FormSettingsResponseSchema,
    pub metadata: FormMetaSchema,
    pub questions: Vec<QuestionResponseSchema>,
    #[schema(value_type = Vec<FormLabelResponseSchema>)]
    pub labels: Vec<FormLabel>,
}

impl FormSchema {
    pub fn from_active_form(form: &ActiveForm, labels: Vec<FormLabel>) -> Self {
        Self {
            id: *form.id(),
            title: form.title().clone(),
            description: form.description().clone(),
            settings: FormSettingsResponseSchema::from_settings_and_answer_settings(
                form.settings(),
                form.answer_settings(),
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
    pub settings: FormSettingsResponseSchema,
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

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct RedmineUserSnapshotResponse {
    redmine_user_id: Option<i64>,
    display_name: String,
}

impl From<RedmineUserSnapshot> for RedmineUserSnapshotResponse {
    fn from(value: RedmineUserSnapshot) -> Self {
        Self {
            redmine_user_id: *value.redmine_user_id(),
            display_name: value.display_name().to_owned(),
        }
    }
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
    #[serde(rename = "IMPORTED_FROM_REDMINE")]
    ImportedFromRedmine {
        redmine_user: RedmineUserSnapshotResponse,
    },
    #[serde(rename = "ANONYMOUS")]
    Anonymous,
}

impl From<PublishedAnswerAuthor> for AnswerAuthor {
    fn from(val: PublishedAnswerAuthor) -> Self {
        match val {
            PublishedAnswerAuthor::AuthenticatedUser(user) => {
                AnswerAuthor::AuthenticatedUser { user: user.into() }
            }
            PublishedAnswerAuthor::Temporary(temporary_user) => AnswerAuthor::Temporary {
                temporary_user: temporary_user.into(),
            },
            PublishedAnswerAuthor::ImportedFromRedmine(author) => {
                AnswerAuthor::ImportedFromRedmine {
                    redmine_user: author.into(),
                }
            }
            PublishedAnswerAuthor::Anonymous => AnswerAuthor::Anonymous,
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
#[serde(rename_all = "UPPERCASE")]
pub enum AnswerCommentSource {
    #[serde(rename = "PORTAL")]
    Portal,
    #[serde(rename = "IMPORTED_FROM_REDMINE")]
    ImportedFromRedmine,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct AnswerComment {
    #[schema(value_type = String, format = "uuid")]
    id: CommentId,
    content: String,
    timestamp: DateTime<Utc>,
    source: AnswerCommentSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    commented_by: Option<User>,
    #[serde(skip_serializing_if = "Option::is_none")]
    redmine_journal_id: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    redmine_author_snapshot: Option<RedmineUserSnapshotResponse>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct HistoryUser {
    #[schema(value_type = String, format = "uuid")]
    id: String,
    name: String,
    role: Role,
}

impl From<&UserSnapshot> for HistoryUser {
    fn from(value: &UserSnapshot) -> Self {
        Self {
            id: value.id().to_string(),
            name: value.name().to_owned(),
            role: value.role().to_owned().into(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum HistoryAction {
    Create,
    Update,
    Delete,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct CommentHistoryResponseEntry {
    #[schema(value_type = String, format = "uuid")]
    id: String,
    #[schema(value_type = String, format = "uuid")]
    comment_id: String,
    original_author: HistoryUser,
    original_timestamp: DateTime<Utc>,
    action: HistoryAction,
    content: String,
    operated_by: HistoryUser,
    operated_at: DateTime<Utc>,
}

impl From<CommentHistoryEntry> for CommentHistoryResponseEntry {
    fn from(value: CommentHistoryEntry) -> Self {
        let action = match value.action() {
            CommentHistoryAction::Create => HistoryAction::Create,
            CommentHistoryAction::Update => HistoryAction::Update,
            CommentHistoryAction::Delete => HistoryAction::Delete,
        };

        Self {
            id: value.id().to_string(),
            comment_id: value.comment_id().to_string(),
            original_author: value.original_author().into(),
            original_timestamp: *value.original_timestamp(),
            action,
            content: value.content().to_string(),
            operated_by: value.operated_by().into(),
            operated_at: *value.operated_at(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct CommentHistoryPageResponse {
    pub items: Vec<CommentHistoryResponseEntry>,
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct MessageHistoryResponseEntry {
    #[schema(value_type = String, format = "uuid")]
    id: String,
    #[schema(value_type = String, format = "uuid")]
    message_id: String,
    original_author: HistoryUser,
    original_timestamp: DateTime<Utc>,
    action: HistoryAction,
    body: String,
    operated_by: HistoryUser,
    operated_at: DateTime<Utc>,
}

impl From<MessageHistoryEntry> for MessageHistoryResponseEntry {
    fn from(value: MessageHistoryEntry) -> Self {
        let action = match value.action() {
            MessageHistoryAction::Create => HistoryAction::Create,
            MessageHistoryAction::Update => HistoryAction::Update,
            MessageHistoryAction::Delete => HistoryAction::Delete,
        };

        Self {
            id: value.id().to_string(),
            message_id: value.message_id().to_string(),
            original_author: value.original_author().into(),
            original_timestamp: *value.original_timestamp(),
            action,
            body: value.body().as_str().to_owned(),
            operated_by: value.operated_by().into(),
            operated_at: *value.operated_at(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct MessageHistoryPageResponse {
    pub items: Vec<MessageHistoryResponseEntry>,
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct AnswerStatusHistoryResponseEntry {
    #[schema(value_type = String, format = "uuid")]
    id: String,
    #[serde(rename = "from")]
    from_status: AnswerStatus,
    #[serde(rename = "to")]
    to_status: AnswerStatus,
    changed_by: HistoryUser,
    changed_at: DateTime<Utc>,
}

impl From<AnswerStatusHistoryEntry> for AnswerStatusHistoryResponseEntry {
    fn from(value: AnswerStatusHistoryEntry) -> Self {
        Self {
            id: value.id().to_string(),
            from_status: (*value.from_status()).into(),
            to_status: (*value.to_status()).into(),
            changed_by: value.changed_by().into(),
            changed_at: *value.changed_at(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct AnswerStatusHistoryPageResponse {
    pub items: Vec<AnswerStatusHistoryResponseEntry>,
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct AnswerTitleHistoryResponseEntry {
    #[schema(value_type = String, format = "uuid")]
    id: String,
    #[serde(rename = "from")]
    from_title: Option<String>,
    #[serde(rename = "to")]
    to_title: Option<String>,
    changed_by: HistoryUser,
    changed_at: DateTime<Utc>,
}

impl From<AnswerTitleHistoryEntry> for AnswerTitleHistoryResponseEntry {
    fn from(value: AnswerTitleHistoryEntry) -> Self {
        Self {
            id: value.id().to_string(),
            from_title: value
                .from_title()
                .clone()
                .into_inner()
                .map(|title| title.into_inner()),
            to_title: value
                .to_title()
                .clone()
                .into_inner()
                .map(|title| title.into_inner()),
            changed_by: value.changed_by().into(),
            changed_at: *value.changed_at(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct AnswerTitleHistoryPageResponse {
    pub items: Vec<AnswerTitleHistoryResponseEntry>,
    pub next_cursor: Option<String>,
}

impl From<CommentWithAuthor> for AnswerComment {
    fn from(val: CommentWithAuthor) -> Self {
        let (source, commented_by, redmine_journal_id, redmine_author_snapshot) =
            match val.commented_by {
                CommentAuthor::Portal(user) => {
                    (AnswerCommentSource::Portal, Some(user.into()), None, None)
                }
                CommentAuthor::ImportedFromRedmine(author) => (
                    AnswerCommentSource::ImportedFromRedmine,
                    None,
                    val.comment.redmine_journal_id(),
                    Some(author.into()),
                ),
            };
        AnswerComment {
            id: val.comment.comment_id().to_owned(),
            content: val.comment.content().to_string(),
            timestamp: val.comment.timestamp().to_owned(),
            source,
            commented_by,
            redmine_journal_id,
            redmine_author_snapshot,
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

#[derive(Debug, utoipa::ToSchema)]
pub struct FormAnswer {
    id: Uuid,
    #[schema(required = false)]
    author: AnswerAuthor,
    form_id: Uuid,
    #[schema(required = false)]
    timestamp: DateTime<Utc>,
    title: Option<String>,
    #[schema(required = false)]
    publication: AnswerPublication,
    #[schema(required = false)]
    status: AnswerStatus,
    answers: Vec<AnswerContent>,
    #[schema(required = false)]
    labels: Vec<AnswerLabels>,
    redmine_issue_id: Option<i64>,
    #[schema(ignore)]
    redacted: bool,
}

impl Serialize for FormAnswer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state =
            serializer.serialize_struct("FormAnswer", if self.redacted { 3 } else { 10 })?;
        state.serialize_field("id", &self.id)?;
        if !self.redacted {
            state.serialize_field("author", &self.author)?;
        }
        state.serialize_field("form_id", &self.form_id)?;
        if !self.redacted {
            state.serialize_field("timestamp", &self.timestamp)?;
            state.serialize_field("title", &self.title)?;
            state.serialize_field("publication", &self.publication)?;
            state.serialize_field("status", &self.status)?;
        }
        state.serialize_field("answers", &self.answers)?;
        if !self.redacted {
            state.serialize_field("labels", &self.labels)?;
            state.serialize_field("redmine_issue_id", &self.redmine_issue_id)?;
        }
        state.end()
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct RelatedAnswerResponse {
    pub form_id: Uuid,
    pub answer_id: Uuid,
}

impl From<AnswerReference> for RelatedAnswerResponse {
    fn from(reference: AnswerReference) -> Self {
        Self {
            form_id: reference.form_id().into_inner(),
            answer_id: reference.answer_id().into_inner(),
        }
    }
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct AnswerListPageResponse {
    pub items: Vec<FormAnswer>,
    pub next_cursor: Option<String>,
}

impl FormAnswer {
    pub fn new(
        answer: PublishedAnswerEntry,
        form_id: FormId,
        labels: Vec<AnswerLabel>,
        answer_response_visibility: AnswerResponseVisibility,
    ) -> Self {
        FormAnswer {
            id: answer.id.into(),
            author: answer.author.into(),
            form_id: form_id.into_inner(),
            timestamp: answer.timestamp,
            title: answer.title.into_inner().map(|title| title.to_string()),
            publication: answer.publication.into(),
            status: answer.status.into(),
            answers: answer
                .contents
                .iter()
                .map(AnswerContent::from_ref)
                .collect_vec(),
            labels: labels.into_iter().map(Into::into).collect_vec(),
            redmine_issue_id: answer
                .redmine_reference
                .map(|reference| reference.issue_id().into_inner()),
            redacted: answer_response_visibility == AnswerResponseVisibility::RESTRICTED,
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
    use domain::form::answer::{
        AnswerId, AnswerTitle, RedmineImportedAnswerReference, RedmineUserSnapshot,
    };
    use domain::form::comment::{Comment, CommentContent, CommentId};
    use domain::form::models::DiscordWebhookUrl;
    use domain::form::question::{Choice, Question};
    use types::non_empty_string::NonEmptyString;
    use types::non_empty_vec::NonEmptyVec;
    use usecase::models::{CommentAuthor, CommentWithAuthor};

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

    #[test]
    fn form_settings_response_exposes_only_whether_the_webhook_is_enabled() {
        let secret = "super-secret-token";
        let settings = FormSettings::new().change_discord_webhook_url(
            DiscordWebhookUrl::try_new(Some(
                NonEmptyString::try_new(format!("https://discord.com/api/webhooks/123/{secret}"))
                    .unwrap(),
            ))
            .unwrap(),
        );
        let schema = FormSettingsResponseSchema::from_settings_and_answer_settings(
            &settings,
            &AnswerSettings::default(),
        );
        let serialized = serde_json::to_value(schema).unwrap();
        let serialized_without_webhook = serde_json::to_value(
            FormSettingsResponseSchema::from_settings_and_answer_settings(
                &FormSettings::new(),
                &AnswerSettings::default(),
            ),
        )
        .unwrap();

        assert_eq!(serialized["discord_webhook_enabled"], true);
        assert!(serialized.get("discord_webhook_url").is_none());
        assert!(!serialized.to_string().contains(secret));
        assert_eq!(serialized_without_webhook["discord_webhook_enabled"], false);
        assert!(
            serialized_without_webhook
                .get("discord_webhook_url")
                .is_none()
        );
    }

    #[test]
    fn anonymous_answer_author_uses_the_anonymous_api_variant() {
        let answer = PublishedAnswerEntry {
            id: AnswerId::from(Uuid::new_v4()),
            author: PublishedAnswerAuthor::Anonymous,
            timestamp: Utc::now(),
            title: AnswerTitle::new(None),
            publication: DomainAnswerPublication::PUBLIC,
            status: DomainAnswerStatus::UNADDRESSED,
            contents: vec![],
            redmine_reference: None,
        };

        let serialized = serde_json::to_value(FormAnswer::new(
            answer,
            FormId::from(Uuid::new_v4()),
            vec![],
            AnswerResponseVisibility::FULL,
        ))
        .unwrap();

        assert_eq!(serialized["author"]["type"], "ANONYMOUS");
        assert_eq!(serialized["author"].as_object().unwrap().len(), 1);
        assert_eq!(serialized["publication"], "PUBLIC");
    }

    #[test]
    fn restricted_answer_response_contains_only_resource_ids_and_answer_values() {
        let answer = PublishedAnswerEntry {
            id: AnswerId::from(Uuid::from_u128(1)),
            author: PublishedAnswerAuthor::Anonymous,
            timestamp: Utc::now(),
            title: AnswerTitle::new(Some("management title".to_string().try_into().unwrap())),
            publication: DomainAnswerPublication::PRIVATE,
            status: DomainAnswerStatus::COMPLETED,
            contents: vec![FormAnswerContent {
                id: Uuid::from_u128(3).into(),
                question_id: Uuid::from_u128(4).into(),
                answer: "input value".to_string(),
            }],
            redmine_reference: None,
        };

        let serialized = serde_json::to_value(FormAnswer::new(
            answer,
            FormId::from(Uuid::from_u128(2)),
            vec![],
            AnswerResponseVisibility::RESTRICTED,
        ))
        .unwrap();

        assert_eq!(serialized.as_object().unwrap().len(), 3);
        assert!(serialized.get("id").is_some());
        assert!(serialized.get("form_id").is_some());
        assert!(serialized.get("answers").is_some());
        assert_eq!(serialized["answers"][0]["answer"], "input value");
        for hidden in [
            "author",
            "timestamp",
            "title",
            "publication",
            "status",
            "labels",
            "redmine_issue_id",
        ] {
            assert!(serialized.get(hidden).is_none(), "field {hidden} leaked");
        }
    }

    #[test]
    fn imported_answer_and_comment_expose_redmine_source_metadata() {
        let answer_id = AnswerId::from(Uuid::new_v4());
        let answer = PublishedAnswerEntry {
            id: answer_id,
            author: PublishedAnswerAuthor::ImportedFromRedmine(RedmineUserSnapshot::new(
                Some(17),
                "Redmine author".to_string(),
            )),
            timestamp: Utc::now(),
            title: AnswerTitle::new(None),
            publication: DomainAnswerPublication::PUBLIC,
            status: DomainAnswerStatus::UNADDRESSED,
            contents: vec![],
            redmine_reference: Some(RedmineImportedAnswerReference::new(answer_id, 1234.into())),
        };
        let comment = Comment::imported_from_redmine(
            answer_id,
            CommentId::new(),
            5678,
            RedmineUserSnapshot::new(None, "Redmine commenter".to_string()),
            CommentContent::new("imported comment".to_string().try_into().unwrap()),
            Utc::now(),
        );

        let answer_json = serde_json::to_value(FormAnswer::new(
            answer,
            FormId::from(Uuid::new_v4()),
            vec![],
            AnswerResponseVisibility::FULL,
        ))
        .unwrap();
        let comment_json = serde_json::to_value(AnswerComment::from(CommentWithAuthor {
            comment,
            commented_by: CommentAuthor::ImportedFromRedmine(RedmineUserSnapshot::new(
                None,
                "Redmine commenter".to_string(),
            )),
        }))
        .unwrap();

        assert_eq!(answer_json["author"]["type"], "IMPORTED_FROM_REDMINE");
        assert_eq!(
            answer_json["author"]["redmine_user"]["display_name"],
            "Redmine author"
        );
        assert_eq!(answer_json["redmine_issue_id"], 1234);
        assert_eq!(comment_json["source"], "IMPORTED_FROM_REDMINE");
        assert_eq!(comment_json["redmine_journal_id"], 5678);
        assert_eq!(
            comment_json["redmine_author_snapshot"]["display_name"],
            "Redmine commenter"
        );
        assert!(comment_json.get("commented_by").is_none());
    }
}
