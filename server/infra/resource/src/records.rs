use std::str::FromStr;

use chrono::{DateTime, Utc};
use domain::{
    account::models::{
        AccountUser, DiscordUser, DiscordUserId, DiscordUserName, Role, UserGroupId,
    },
    form::answer::TemporaryAnswerAuthor,
    form::{
        answer::{AnswerAuthor, AnswerEntry, AnswerLabel, AnswerTitle, FormAnswerContent},
        comment::{Comment, CommentContent},
        message::{Message, MessageBody},
        models::{
            ActiveForm, AllowedUserGroups, AnswerAcceptancePeriod, AnswerSettings, ArchivedForm,
            DefaultAnswerTitle, DiscordWebhookUrl, FormDescription, FormId, FormLabel,
            FormLabelAssignment, FormLabelId, FormLabelName, FormMeta, FormSettings, FormTitle,
            QuestionSet,
        },
        question::{Choice, Question, QuestionType},
    },
    notification::models::NotificationPreference,
};
use errors::{Error, infra::InfraError};
use types::non_empty_string::NonEmptyString;
use types::non_empty_vec::NonEmptyVec;
use uuid::Uuid;

#[derive(Clone)]
pub struct ChoiceRecord {
    pub id: Option<i32>,
    pub position: u16,
    pub label: String,
}

impl TryFrom<ChoiceRecord> for Choice {
    type Error = Error;

    fn try_from(
        ChoiceRecord {
            id,
            position,
            label,
        }: ChoiceRecord,
    ) -> Result<Self, Self::Error> {
        unsafe {
            Choice::from_raw_parts(id.map(Into::into), position, label.try_into()?)
                .map_err(Into::into)
        }
    }
}

#[derive(Clone)]
pub struct QuestionRecord {
    pub id: String,
    pub form_id: String,
    pub template_key: String,
    pub position: u16,
    pub title: String,
    pub description: Option<String>,
    pub question_type: String,
    pub choices: Vec<ChoiceRecord>,
    pub is_required: bool,
}

impl TryFrom<QuestionRecord> for Question {
    type Error = Error;

    fn try_from(
        QuestionRecord {
            id,
            form_id: _,
            template_key,
            position,
            title,
            description,
            question_type,
            choices,
            is_required,
        }: QuestionRecord,
    ) -> Result<Self, Self::Error> {
        let choices = choices
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, _>>()
            .map(|choices| {
                (!choices.is_empty())
                    .then(|| NonEmptyVec::try_new(choices).expect("non-empty choices"))
            })?;

        unsafe {
            Question::from_raw_parts(
                Uuid::from_str(&id)
                    .map_err(Into::<InfraError>::into)?
                    .into(),
                template_key.try_into()?,
                position,
                title.try_into()?,
                description.map(TryInto::try_into).transpose()?,
                QuestionType::from_str(&question_type).map_err(Into::<InfraError>::into)?,
                choices,
                is_required,
            )
            .map_err(Into::into)
        }
    }
}

pub struct ActiveFormRecord {
    pub id: String,
    pub title: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub discord_webhook_url: Option<String>,
    pub visibility: String,
    pub answer_visibility: String,
    pub allow_temporary_answers: bool,
    pub acceptance_period_start_at: Option<DateTime<Utc>>,
    pub acceptance_period_end_at: Option<DateTime<Utc>>,
    pub default_answer_title: Option<String>,
    pub allowed_group_ids: Vec<UserGroupId>,
    pub answer_group_ids: Vec<UserGroupId>,
    pub questions: Vec<QuestionRecord>,
    pub label_ids: Vec<FormLabelId>,
}

impl TryFrom<ActiveFormRecord> for ActiveForm {
    type Error = Error;

    fn try_from(
        ActiveFormRecord {
            id,
            title,
            description,
            created_at,
            updated_at,
            discord_webhook_url,
            visibility,
            answer_visibility,
            allow_temporary_answers,
            acceptance_period_start_at,
            acceptance_period_end_at,
            default_answer_title,
            allowed_group_ids,
            answer_group_ids,
            questions,
            label_ids,
        }: ActiveFormRecord,
    ) -> Result<Self, Self::Error> {
        let questions = questions
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, _>>()?;
        let questions = NonEmptyVec::try_new(questions).map_err(Error::from)?;

        let answer_settings = AnswerSettings::new(
            DefaultAnswerTitle::new(
                default_answer_title
                    .map(NonEmptyString::try_new)
                    .transpose()?,
            ),
            answer_visibility.try_into()?,
            AnswerAcceptancePeriod::try_new(acceptance_period_start_at, acceptance_period_end_at)?,
            allow_temporary_answers,
        )
        .change_answer_groups(AllowedUserGroups::new(answer_group_ids));

        Ok(unsafe {
            ActiveForm::from_raw_parts(
                FormId::from(Uuid::parse_str(&id).map_err(Into::<InfraError>::into)?),
                FormTitle::new(title.try_into()?),
                FormDescription::new(description),
                FormMeta::from_raw_parts(created_at, updated_at),
                FormSettings::from_raw_parts(
                    DiscordWebhookUrl::try_new(
                        discord_webhook_url
                            .map(NonEmptyString::try_new)
                            .transpose()?,
                    )?,
                    visibility.try_into()?,
                    AllowedUserGroups::new(allowed_group_ids),
                ),
                answer_settings,
                QuestionSet::try_new(questions)?,
                FormLabelAssignment::try_new(label_ids)?,
            )
        })
    }
}

pub struct ArchivedFormRecord {
    pub form: ActiveFormRecord,
    pub archived_at: DateTime<Utc>,
    pub archived_by_name: String,
    pub archived_by_id: String,
    pub archived_by_role: Role,
}

impl TryFrom<ArchivedFormRecord> for ArchivedForm {
    type Error = Error;

    fn try_from(value: ArchivedFormRecord) -> Result<Self, Self::Error> {
        let form = value.form.try_into()?;
        let archived_by = Uuid::from_str(&value.archived_by_id)
            .map_err(Into::<InfraError>::into)?
            .into();

        Ok(unsafe { ArchivedForm::from_raw_parts(form, value.archived_at, archived_by) })
    }
}

#[derive(Clone)]
pub struct FormAnswerContentRecord {
    pub id: String,
    pub question_id: String,
    pub answer: String,
}

impl TryFrom<FormAnswerContentRecord> for FormAnswerContent {
    type Error = InfraError;

    fn try_from(
        FormAnswerContentRecord {
            id,
            question_id,
            answer,
        }: FormAnswerContentRecord,
    ) -> Result<Self, Self::Error> {
        Ok(FormAnswerContent {
            id: Uuid::parse_str(&id)?.into(),
            question_id: Uuid::parse_str(&question_id)?.into(),
            answer,
        })
    }
}

pub struct UserRecord {
    pub name: String,
    pub id: String,
    pub role: Role,
}

impl TryFrom<UserRecord> for AccountUser {
    type Error = InfraError;

    fn try_from(UserRecord { name, id, role }: UserRecord) -> Result<Self, Self::Error> {
        Ok(AccountUser::new(name, Uuid::from_str(&id)?.into(), role))
    }
}

#[derive(Clone)]
pub struct CommentRecord {
    pub answer_id: String,
    pub comment_id: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub commented_by_name: String,
    pub commented_by_id: String,
    pub commented_by_role: String,
}

pub struct CommentHistoryRecord {
    pub id: String,
    pub answer_id: String,
    pub comment_id: String,
    pub original_author_id: String,
    pub original_author_name: String,
    pub original_author_role: String,
    pub original_timestamp: DateTime<Utc>,
    pub action: String,
    pub content: String,
    pub operated_by_id: String,
    pub operated_by_name: String,
    pub operated_by_role: String,
    pub operated_at: DateTime<Utc>,
}

impl TryFrom<CommentRecord> for Comment {
    type Error = Error;

    fn try_from(
        CommentRecord {
            answer_id,
            comment_id,
            content,
            timestamp,
            commented_by_name: _,
            commented_by_id,
            commented_by_role: _,
        }: CommentRecord,
    ) -> Result<Self, Self::Error> {
        Ok(unsafe {
            Comment::from_raw_parts(
                Uuid::from_str(&answer_id)
                    .map_err(Into::<InfraError>::into)?
                    .into(),
                Uuid::from_str(&comment_id)
                    .map_err(Into::<InfraError>::into)?
                    .into(),
                CommentContent::new(content.try_into()?),
                timestamp,
                Uuid::from_str(&commented_by_id)
                    .map_err(Into::<InfraError>::into)?
                    .into(),
            )
        })
    }
}

pub struct FormAnswerRecord {
    pub id: String,
    pub author: AnswerAuthorRecord,
    pub timestamp: DateTime<Utc>,
    pub form_id: String,
    pub title: Option<String>,
    pub contents: Vec<FormAnswerContentRecord>,
    pub messages: Vec<MessageRecord>,
}

pub enum AnswerAuthorRecord {
    AuthenticatedUser(AccountUser),
    TemporaryAnswerAuthor(TemporaryAnswerAuthor),
}

impl TryFrom<FormAnswerRecord> for AnswerEntry {
    type Error = Error;

    fn try_from(
        FormAnswerRecord {
            id,
            author,
            timestamp,
            form_id,
            title,
            contents,
            messages: _,
        }: FormAnswerRecord,
    ) -> Result<Self, Self::Error> {
        let author = match author {
            AnswerAuthorRecord::AuthenticatedUser(user) => {
                AnswerAuthor::AuthenticatedUser(*user.id())
            }
            AnswerAuthorRecord::TemporaryAnswerAuthor(user) => AnswerAuthor::Temporary(user),
        };
        unsafe {
            Ok(AnswerEntry::from_raw_parts(
                Uuid::from_str(&id)
                    .map_err(Into::<InfraError>::into)?
                    .into(),
                FormId::from(Uuid::from_str(&form_id).map_err(Into::<InfraError>::into)?),
                author,
                timestamp,
                AnswerTitle::new(title.map(TryInto::try_into).transpose()?),
                contents
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<_, _>>()?,
            ))
        }
    }
}

pub struct AnswerLabelRecord {
    pub id: String,
    pub name: String,
}

impl TryFrom<AnswerLabelRecord> for AnswerLabel {
    type Error = Error;

    fn try_from(AnswerLabelRecord { id, name }: AnswerLabelRecord) -> Result<Self, Self::Error> {
        Ok(unsafe {
            AnswerLabel::from_raw_parts(
                Uuid::from_str(&id)
                    .map_err(Into::<InfraError>::into)?
                    .into(),
                NonEmptyString::try_new(name)?,
            )
        })
    }
}

pub struct FormLabelRecord {
    pub id: String,
    pub name: String,
}

impl TryFrom<FormLabelRecord> for FormLabel {
    type Error = Error;

    fn try_from(FormLabelRecord { id, name }: FormLabelRecord) -> Result<Self, Self::Error> {
        Ok(unsafe {
            FormLabel::from_raw_parts(
                Uuid::from_str(&id)
                    .map_err(Into::<InfraError>::into)?
                    .into(),
                FormLabelName::new(name.try_into()?),
            )
        })
    }
}

#[derive(Clone)]
pub struct MessageRecord {
    pub id: String,
    pub sender_name: String,
    pub sender_id: String,
    pub sender_role: String,
    pub body: String,
    pub timestamp: DateTime<Utc>,
}

pub struct MessageHistoryRecord {
    pub id: String,
    pub answer_id: String,
    pub message_id: String,
    pub original_author_id: String,
    pub original_author_name: String,
    pub original_author_role: String,
    pub original_timestamp: DateTime<Utc>,
    pub action: String,
    pub body: String,
    pub operated_by_id: String,
    pub operated_by_name: String,
    pub operated_by_role: String,
    pub operated_at: DateTime<Utc>,
}

impl TryFrom<MessageRecord> for Message {
    type Error = Error;

    fn try_from(
        MessageRecord {
            id,
            sender_name: _,
            sender_id,
            sender_role: _,
            body,
            timestamp,
        }: MessageRecord,
    ) -> Result<Self, Self::Error> {
        unsafe {
            Ok(Message::from_raw_parts(
                Uuid::from_str(&id)
                    .map_err(Into::<InfraError>::into)?
                    .into(),
                Uuid::from_str(&sender_id)
                    .map_err(Into::<InfraError>::into)?
                    .into(),
                MessageBody::new(body.try_into()?),
                timestamp,
            ))
        }
    }
}

pub struct NotificationSettingsRecord {
    pub recipient: UserRecord,
    pub is_send_message_notification: bool,
}

impl TryFrom<NotificationSettingsRecord> for NotificationPreference {
    type Error = Error;

    fn try_from(
        NotificationSettingsRecord {
            recipient,
            is_send_message_notification,
        }: NotificationSettingsRecord,
    ) -> Result<Self, Self::Error> {
        Ok(unsafe {
            NotificationPreference::from_raw_parts(
                Uuid::from_str(&recipient.id)
                    .map_err(Into::<InfraError>::into)?
                    .into(),
                is_send_message_notification,
            )
        })
    }
}

pub struct DiscordUserRecord {
    pub user_id: String,
    pub username: String,
}

impl From<DiscordUserRecord> for DiscordUser {
    fn from(DiscordUserRecord { user_id, username }: DiscordUserRecord) -> Self {
        DiscordUser::new(DiscordUserId::new(user_id), DiscordUserName::new(username))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn question_record_rejects_text_question_with_choices() {
        let result: Result<Question, _> = QuestionRecord {
            id: Uuid::nil().to_string(),
            form_id: Uuid::nil().to_string(),
            template_key: "template".to_string(),
            position: 0,
            title: "Question".to_string(),
            description: None,
            question_type: "Text".to_string(),
            choices: vec![ChoiceRecord {
                id: Some(1),
                position: 0,
                label: "A".to_string(),
            }],
            is_required: true,
        }
        .try_into();

        assert!(result.is_err());
    }
}
