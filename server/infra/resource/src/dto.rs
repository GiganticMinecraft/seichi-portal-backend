use std::str::FromStr;

use chrono::{DateTime, Utc};
use domain::{
    form::{
        answer::{
            models::AnswerTitle,
            settings::models::{DefaultAnswerTitle, ResponsePeriod},
        },
        comment::models::CommentContent,
        models::{FormDescription, FormId, FormMeta, FormSettings, FormTitle, WebhookUrl},
        question::models::{Question, QuestionType},
    },
    user::models::{Role, User},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct QuestionDto {
    pub id: Option<i32>,
    pub form_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub question_type: String,
    pub choices: Vec<String>,
    pub is_required: bool,
}

impl TryFrom<QuestionDto> for domain::form::question::models::Question {
    type Error = errors::domain::DomainError;

    fn try_from(
        QuestionDto {
            id,
            form_id,
            title,
            description,
            question_type,
            choices,
            is_required,
        }: QuestionDto,
    ) -> Result<Self, Self::Error> {
        Ok(Question::from_raw_parts(
            id.map(Into::into),
            FormId::from(form_id),
            title,
            description,
            QuestionType::from_str(&question_type)?,
            choices,
            is_required,
        ))
    }
}

pub struct FormDto {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub metadata: (DateTime<Utc>, DateTime<Utc>),
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
    pub webhook_url: Option<String>,
    pub default_answer_title: Option<String>,
    pub visibility: String,
    pub answer_visibility: String,
}

impl TryFrom<FormDto> for domain::form::models::Form {
    type Error = errors::Error;

    fn try_from(
        FormDto {
            id,
            title,
            description,
            metadata,
            start_at,
            end_at,
            webhook_url,
            default_answer_title,
            visibility,
            answer_visibility,
        }: FormDto,
    ) -> Result<Self, Self::Error> {
        Ok(domain::form::models::Form::from_raw_parts(
            FormId::from(id),
            FormTitle::new(title.try_into()?),
            FormDescription::new(description.map(TryInto::try_into).transpose()?),
            FormMeta::from_raw_parts(metadata.0, metadata.1),
            FormSettings::from_raw_parts(
                ResponsePeriod::try_new(start_at, end_at)?,
                WebhookUrl::try_new(webhook_url.map(TryInto::try_into).transpose()?)?,
                DefaultAnswerTitle::new(default_answer_title.map(TryInto::try_into).transpose()?),
                visibility.try_into()?,
                answer_visibility.try_into()?,
            ),
        ))
    }
}

pub struct FormAnswerContentDto {
    pub question_id: i32,
    pub answer: String,
}

impl TryFrom<FormAnswerContentDto> for domain::form::answer::models::FormAnswerContent {
    type Error = errors::domain::DomainError;

    fn try_from(
        FormAnswerContentDto {
            question_id,
            answer,
        }: FormAnswerContentDto,
    ) -> Result<Self, Self::Error> {
        Ok(domain::form::answer::models::FormAnswerContent {
            question_id: question_id.into(),
            answer,
        })
    }
}

pub struct UserDto {
    pub name: String,
    pub id: Uuid,
    pub role: Role,
}

impl TryFrom<UserDto> for User {
    type Error = errors::domain::DomainError;

    fn try_from(UserDto { name, id, role }: UserDto) -> Result<Self, Self::Error> {
        Ok(User { name, id, role })
    }
}

pub struct CommentDto {
    pub answer_id: Uuid,
    pub comment_id: Uuid,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub commented_by: UserDto,
}

impl TryFrom<CommentDto> for domain::form::comment::models::Comment {
    type Error = errors::Error;

    fn try_from(
        CommentDto {
            answer_id,
            comment_id,
            content,
            timestamp,
            commented_by,
        }: CommentDto,
    ) -> Result<Self, Self::Error> {
        Ok(domain::form::comment::models::Comment::from_raw_parts(
            answer_id.into(),
            comment_id.into(),
            CommentContent::new(content.try_into()?),
            timestamp,
            commented_by.try_into()?,
        ))
    }
}

pub struct FormAnswerDto {
    pub id: Uuid,
    pub user_name: String,
    pub uuid: Uuid,
    pub user_role: Role,
    pub timestamp: DateTime<Utc>,
    pub form_id: Uuid,
    pub title: Option<String>,
}

impl TryFrom<FormAnswerDto> for domain::form::answer::models::AnswerEntry {
    type Error = errors::Error;

    fn try_from(
        FormAnswerDto {
            id,
            user_name,
            uuid,
            user_role,
            timestamp,
            form_id,
            title,
        }: FormAnswerDto,
    ) -> Result<Self, Self::Error> {
        unsafe {
            Ok(domain::form::answer::models::AnswerEntry::from_raw_parts(
                id.into(),
                User {
                    name: user_name,
                    id: uuid,
                    role: user_role,
                },
                timestamp,
                FormId::from(form_id),
                AnswerTitle::new(title.map(TryInto::try_into).transpose()?),
            ))
        }
    }
}

pub struct AnswerLabelDto {
    pub id: i32,
    pub name: String,
}

impl TryFrom<AnswerLabelDto> for domain::form::answer::models::AnswerLabel {
    type Error = errors::domain::DomainError;

    fn try_from(AnswerLabelDto { id, name }: AnswerLabelDto) -> Result<Self, Self::Error> {
        Ok(domain::form::answer::models::AnswerLabel {
            id: id.into(),
            name,
        })
    }
}

pub struct FormLabelDto {
    pub id: Uuid,
    pub name: String,
}

impl TryFrom<FormLabelDto> for domain::form::models::FormLabel {
    type Error = errors::validation::ValidationError;

    fn try_from(FormLabelDto { id, name }: FormLabelDto) -> Result<Self, Self::Error> {
        Ok(domain::form::models::FormLabel::from_raw_parts(
            id.into(),
            domain::form::models::FormLabelName::new(name.try_into()?),
        ))
    }
}

pub struct MessageDto {
    pub id: Uuid,
    pub related_answer: FormAnswerDto,
    pub sender: UserDto,
    pub body: String,
    pub timestamp: DateTime<Utc>,
}

impl TryFrom<MessageDto> for domain::form::message::models::Message {
    type Error = errors::Error;

    fn try_from(
        MessageDto {
            id,
            related_answer,
            sender,
            body,
            timestamp,
        }: MessageDto,
    ) -> Result<Self, Self::Error> {
        unsafe {
            Ok(domain::form::message::models::Message::from_raw_parts(
                id.into(),
                related_answer.try_into()?,
                sender.try_into()?,
                body,
                timestamp,
            ))
        }
    }
}

pub struct NotificationSettingsDto {
    pub recipient: UserDto,
    pub is_send_message_notification: bool,
}

impl TryFrom<NotificationSettingsDto> for domain::notification::models::NotificationSettings {
    type Error = errors::domain::DomainError;

    fn try_from(
        NotificationSettingsDto {
            recipient,
            is_send_message_notification,
        }: NotificationSettingsDto,
    ) -> Result<Self, Self::Error> {
        Ok(
            domain::notification::models::NotificationSettings::from_raw_parts(
                recipient.try_into()?,
                is_send_message_notification,
            ),
        )
    }
}
