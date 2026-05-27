use std::str::FromStr;

use chrono::{DateTime, Utc};
use domain::{
    form::{
        answer::models::{AnswerAuthor, AnswerEntry, AnswerLabel, AnswerTitle, FormAnswerContent},
        answer_entry_set::models::AnswerEntrySetId,
        comment::models::{Comment, CommentContent},
        message::models::Message,
        models::{
            ActiveForm, ArchivedForm, FormDescription, FormId, FormLabel, FormLabelId,
            FormLabelIdSet, FormLabelName, FormMeta, FormSettings, FormTitle, QuestionSet,
            WebhookUrl,
        },
        question::models::{Choice, Question, QuestionType},
    },
    notification::models::NotificationPreference,
    user::models::{ActiveUser, DiscordUser, DiscordUserId, DiscordUserName, Role, TemporaryUser},
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
        Choice::from_raw_parts(id.map(Into::into), position, label.try_into()?).map_err(Into::into)
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

pub struct ActiveFormRecord {
    pub id: String,
    pub title: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub webhook_url: Option<String>,
    pub visibility: String,
    pub questions: Vec<QuestionRecord>,
    pub label_ids: Vec<FormLabelId>,
    pub answer_entry_set_id: String,
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
            webhook_url,
            visibility,
            questions,
            label_ids,
            answer_entry_set_id,
        }: ActiveFormRecord,
    ) -> Result<Self, Self::Error> {
        let questions = questions
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, _>>()?;
        let questions = NonEmptyVec::try_new(questions).map_err(Error::from)?;

        Ok(ActiveForm::from_raw_parts(
            FormId::from(Uuid::parse_str(&id).map_err(Into::<InfraError>::into)?),
            FormTitle::new(title.try_into()?),
            FormDescription::new(description),
            FormMeta::from_raw_parts(created_at, updated_at),
            FormSettings::from_raw_parts(
                WebhookUrl::try_new(webhook_url.map(NonEmptyString::try_new).transpose()?)?,
                visibility.try_into()?,
            ),
            QuestionSet::try_new(questions)?,
            FormLabelIdSet::try_new(label_ids)?,
            AnswerEntrySetId::from(
                Uuid::parse_str(&answer_entry_set_id).map_err(Into::<InfraError>::into)?,
            ),
        ))
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
        Ok(ArchivedForm::from_persisted(
            value.form.try_into()?,
            value.archived_at,
            Uuid::from_str(&value.archived_by_id)
                .map_err(Into::<InfraError>::into)?
                .into(),
        ))
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

impl TryFrom<UserRecord> for ActiveUser {
    type Error = InfraError;

    fn try_from(UserRecord { name, id, role }: UserRecord) -> Result<Self, Self::Error> {
        Ok(ActiveUser::new(name, Uuid::from_str(&id)?.into(), role))
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
        Ok(Comment::from_raw_parts(
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
        ))
    }
}

pub struct FormAnswerRecord {
    pub id: String,
    pub author: AnswerAuthorRecord,
    pub timestamp: DateTime<Utc>,
    pub form_id: String,
    pub title: Option<String>,
    pub contents: Vec<FormAnswerContentRecord>,
    pub comments: Vec<CommentRecord>,
    pub messages: Vec<MessageRecord>,
}

pub enum AnswerAuthorRecord {
    AuthenticatedUser(ActiveUser),
    TemporaryUser(TemporaryUser),
}

impl TryFrom<FormAnswerRecord> for AnswerEntry {
    type Error = Error;

    fn try_from(
        FormAnswerRecord {
            id,
            author,
            timestamp,
            form_id: _,
            title,
            contents,
            comments,
            messages: _,
        }: FormAnswerRecord,
    ) -> Result<Self, Self::Error> {
        let author = match author {
            AnswerAuthorRecord::AuthenticatedUser(user) => {
                AnswerAuthor::AuthenticatedUser(*user.id())
            }
            AnswerAuthorRecord::TemporaryUser(user) => AnswerAuthor::TemporaryUser(user),
        };
        let comments = comments
            .into_iter()
            .map(TryInto::<Comment>::try_into)
            .collect::<Result<Vec<_>, _>>()?;
        unsafe {
            Ok(AnswerEntry::from_raw_parts(
                Uuid::from_str(&id)
                    .map_err(Into::<InfraError>::into)?
                    .into(),
                author,
                timestamp,
                AnswerTitle::new(title.map(TryInto::try_into).transpose()?),
                contents
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<_, _>>()?,
                comments,
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
        Ok(AnswerLabel::from_raw_parts(
            Uuid::from_str(&id)
                .map_err(Into::<InfraError>::into)?
                .into(),
            NonEmptyString::try_new(name)?,
        ))
    }
}

pub struct FormLabelRecord {
    pub id: String,
    pub name: String,
}

impl TryFrom<FormLabelRecord> for FormLabel {
    type Error = Error;

    fn try_from(FormLabelRecord { id, name }: FormLabelRecord) -> Result<Self, Self::Error> {
        Ok(FormLabel::from_raw_parts(
            Uuid::from_str(&id)
                .map_err(Into::<InfraError>::into)?
                .into(),
            FormLabelName::new(name.try_into()?),
        ))
    }
}

#[derive(Clone)]
pub struct MessageRecord {
    pub id: String,
    pub related_answer: String,
    pub sender_name: String,
    pub sender_id: String,
    pub sender_role: String,
    pub body: String,
    pub timestamp: DateTime<Utc>,
}

impl TryFrom<MessageRecord> for Message {
    type Error = Error;

    fn try_from(
        MessageRecord {
            id,
            related_answer,
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
                Uuid::from_str(&related_answer)
                    .map_err(Into::<InfraError>::into)?
                    .into(),
                Uuid::from_str(&sender_id)
                    .map_err(Into::<InfraError>::into)?
                    .into(),
                body,
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
        Ok(NotificationPreference::from_raw_parts(
            Uuid::from_str(&recipient.id)
                .map_err(Into::<InfraError>::into)?
                .into(),
            is_send_message_notification,
        ))
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
