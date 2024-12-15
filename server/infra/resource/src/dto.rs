use std::str::FromStr;

use chrono::{DateTime, Utc};
use domain::{
    form::models::{
        DefaultAnswerTitle, FormDescription, FormId, FormMeta, FormSettings, FormTitle, Question,
        QuestionType, ResponsePeriod, WebhookUrl,
    },
    user::models::{Role, User},
};
use strum_macros::{Display, EnumString};
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

impl TryFrom<QuestionDto> for domain::form::models::Question {
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
    type Error = errors::domain::DomainError;

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
            FormTitle::try_new(title)?,
            FormDescription::try_new(description)?,
            FormMeta::from_raw_parts(metadata.0, metadata.1),
            FormSettings::from_raw_parts(
                ResponsePeriod::try_new(start_at, end_at)?,
                WebhookUrl::try_new(webhook_url)?,
                DefaultAnswerTitle::try_new(default_answer_title)?,
                visibility.try_into()?,
                answer_visibility.try_into()?,
            ),
        ))
    }
}

pub struct FormAnswerContentDto {
    pub answer_id: i32,
    pub question_id: i32,
    pub answer: String,
}

impl TryFrom<FormAnswerContentDto> for domain::form::models::FormAnswerContent {
    type Error = errors::domain::DomainError;

    fn try_from(
        FormAnswerContentDto {
            answer_id,
            question_id,
            answer,
        }: FormAnswerContentDto,
    ) -> Result<Self, Self::Error> {
        Ok(domain::form::models::FormAnswerContent {
            answer_id: answer_id.into(),
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
    pub answer_id: i32,
    pub comment_id: i32,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    pub commented_by: UserDto,
}

impl TryFrom<CommentDto> for domain::form::models::Comment {
    type Error = errors::domain::DomainError;

    fn try_from(
        CommentDto {
            answer_id,
            comment_id,
            content,
            timestamp,
            commented_by,
        }: CommentDto,
    ) -> Result<Self, Self::Error> {
        Ok(domain::form::models::Comment {
            answer_id: answer_id.into(),
            comment_id: comment_id.into(),
            content,
            timestamp,
            commented_by: commented_by.try_into()?,
        })
    }
}

pub struct FormAnswerDto {
    pub id: i32,
    pub user_name: String,
    pub uuid: Uuid,
    pub user_role: Role,
    pub timestamp: DateTime<Utc>,
    pub form_id: Uuid,
    pub title: Option<String>,
}

impl TryFrom<FormAnswerDto> for domain::form::models::FormAnswer {
    type Error = errors::domain::DomainError;

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
        Ok(domain::form::models::FormAnswer {
            id: id.into(),
            user: User {
                name: user_name,
                id: uuid,
                role: user_role,
            },
            timestamp,
            form_id: FormId::from(form_id),
            title,
        })
    }
}

pub struct AnswerLabelDto {
    pub id: i32,
    pub answer_id: i32,
    pub name: String,
}

impl TryFrom<AnswerLabelDto> for domain::form::models::AnswerLabel {
    type Error = errors::domain::DomainError;

    fn try_from(
        AnswerLabelDto {
            id,
            answer_id,
            name,
        }: AnswerLabelDto,
    ) -> Result<Self, Self::Error> {
        Ok(domain::form::models::AnswerLabel {
            id: id.into(),
            answer_id: answer_id.into(),
            name,
        })
    }
}

pub struct LabelDto {
    pub id: i32,
    pub name: String,
}

impl TryFrom<LabelDto> for domain::form::models::Label {
    type Error = errors::domain::DomainError;

    fn try_from(LabelDto { id, name }: LabelDto) -> Result<Self, Self::Error> {
        Ok(domain::form::models::Label {
            id: id.into(),
            name,
        })
    }
}

pub struct MessageDto {
    pub id: Uuid,
    pub related_answer: FormAnswerDto,
    pub sender: UserDto,
    pub body: String,
    pub timestamp: DateTime<Utc>,
}

impl TryFrom<MessageDto> for domain::form::models::Message {
    type Error = errors::domain::DomainError;

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
            Ok(domain::form::models::Message::from_raw_parts(
                id.into(),
                related_answer.try_into()?,
                sender.try_into()?,
                body,
                timestamp,
            ))
        }
    }
}

#[derive(Debug, EnumString, Display)]
pub enum NotificationSourceTypeDto {
    #[strum(serialize = "MESSAGE")]
    Message,
}

pub struct NotificationSourceInformationDto {
    pub source_type: NotificationSourceTypeDto,
    pub source_id: Uuid,
}

impl TryFrom<NotificationSourceInformationDto>
    for domain::notification::models::NotificationSource
{
    type Error = errors::domain::DomainError;

    fn try_from(
        NotificationSourceInformationDto {
            source_type,
            source_id,
        }: NotificationSourceInformationDto,
    ) -> Result<Self, Self::Error> {
        match source_type {
            NotificationSourceTypeDto::Message => Ok(
                domain::notification::models::NotificationSource::Message(source_id.into()),
            ),
        }
    }
}

pub struct NotificationDto {
    pub id: Uuid,
    pub source: NotificationSourceInformationDto,
    pub recipient: UserDto,
    pub is_read: bool,
}

impl TryFrom<NotificationDto> for domain::notification::models::Notification {
    type Error = errors::domain::DomainError;

    fn try_from(
        NotificationDto {
            id,
            source,
            recipient,
            is_read,
        }: NotificationDto,
    ) -> Result<Self, Self::Error> {
        unsafe {
            Ok(domain::notification::models::Notification::from_raw_parts(
                id.into(),
                source.try_into()?,
                recipient.try_into()?,
                is_read,
            ))
        }
    }
}
