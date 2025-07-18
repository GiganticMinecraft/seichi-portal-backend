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
use errors::infra::InfraError;
use types::non_empty_string::NonEmptyString;
use uuid::Uuid;

#[derive(Clone)]
pub struct QuestionDto {
    pub id: Option<i32>,
    pub form_id: String,
    pub title: String,
    pub description: Option<String>,
    pub question_type: String,
    pub choices: Vec<String>,
    pub is_required: bool,
}

impl TryFrom<QuestionDto> for domain::form::question::models::Question {
    type Error = errors::Error;

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
            FormId::from(Uuid::from_str(&form_id).map_err(Into::<InfraError>::into)?),
            title,
            description,
            QuestionType::from_str(&question_type).map_err(Into::<InfraError>::into)?,
            choices,
            is_required,
        ))
    }
}

pub struct FormDto {
    pub id: String,
    pub title: String,
    pub description: String,
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
            FormId::from(Uuid::from_str(&id).map_err(Into::<InfraError>::into)?),
            FormTitle::new(title.try_into()?),
            FormDescription::new(description),
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

#[derive(Clone)]
pub struct FormAnswerContentDto {
    pub id: String,
    pub question_id: i32,
    pub answer: String,
}

impl TryFrom<FormAnswerContentDto> for domain::form::answer::models::FormAnswerContent {
    type Error = InfraError;

    fn try_from(
        FormAnswerContentDto {
            id,
            question_id,
            answer,
        }: FormAnswerContentDto,
    ) -> Result<Self, Self::Error> {
        Ok(domain::form::answer::models::FormAnswerContent {
            id: Uuid::parse_str(&id)?.into(),
            question_id: question_id.into(),
            answer,
        })
    }
}

pub struct UserDto {
    pub name: String,
    pub id: String,
    pub role: Role,
}

impl TryFrom<UserDto> for User {
    type Error = errors::infra::InfraError;

    fn try_from(UserDto { name, id, role }: UserDto) -> Result<Self, Self::Error> {
        Ok(User {
            name,
            id: Uuid::from_str(&id)?,
            role,
        })
    }
}

pub struct CommentDto {
    pub answer_id: String,
    pub comment_id: String,
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
            Uuid::from_str(&answer_id)
                .map_err(Into::<InfraError>::into)?
                .into(),
            Uuid::from_str(&comment_id)
                .map_err(Into::<InfraError>::into)?
                .into(),
            CommentContent::new(content.try_into()?),
            timestamp,
            commented_by.try_into()?,
        ))
    }
}

pub struct FormAnswerDto {
    pub id: String,
    pub user_name: String,
    pub uuid: String,
    pub user_role: Role,
    pub timestamp: DateTime<Utc>,
    pub form_id: String,
    pub title: Option<String>,
    pub contents: Vec<FormAnswerContentDto>,
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
            contents,
        }: FormAnswerDto,
    ) -> Result<Self, Self::Error> {
        unsafe {
            Ok(domain::form::answer::models::AnswerEntry::from_raw_parts(
                Uuid::from_str(&id)
                    .map_err(Into::<InfraError>::into)?
                    .into(),
                User {
                    name: user_name,
                    id: Uuid::from_str(&uuid).map_err(Into::<InfraError>::into)?,
                    role: user_role,
                },
                timestamp,
                FormId::from(Uuid::from_str(&form_id).map_err(Into::<InfraError>::into)?),
                AnswerTitle::new(title.map(TryInto::try_into).transpose()?),
                contents
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<_, _>>()?,
            ))
        }
    }
}

pub struct AnswerLabelDto {
    pub id: String,
    pub name: String,
}

impl TryFrom<AnswerLabelDto> for domain::form::answer::models::AnswerLabel {
    type Error = errors::Error;

    fn try_from(AnswerLabelDto { id, name }: AnswerLabelDto) -> Result<Self, Self::Error> {
        Ok(domain::form::answer::models::AnswerLabel::from_raw_parts(
            Uuid::from_str(&id)
                .map_err(Into::<InfraError>::into)?
                .into(),
            NonEmptyString::try_new(name)?,
        ))
    }
}

pub struct FormLabelDto {
    pub id: String,
    pub name: String,
}

impl TryFrom<FormLabelDto> for domain::form::models::FormLabel {
    type Error = errors::Error;

    fn try_from(FormLabelDto { id, name }: FormLabelDto) -> Result<Self, Self::Error> {
        Ok(domain::form::models::FormLabel::from_raw_parts(
            Uuid::from_str(&id)
                .map_err(Into::<InfraError>::into)?
                .into(),
            domain::form::models::FormLabelName::new(name.try_into()?),
        ))
    }
}

pub struct MessageDto {
    pub id: String,
    pub related_answer: String,
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
                Uuid::from_str(&id)
                    .map_err(Into::<InfraError>::into)?
                    .into(),
                Uuid::from_str(&related_answer)
                    .map_err(Into::<InfraError>::into)?
                    .into(),
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

impl TryFrom<NotificationSettingsDto> for domain::notification::models::NotificationPreference {
    type Error = errors::Error;

    fn try_from(
        NotificationSettingsDto {
            recipient,
            is_send_message_notification,
        }: NotificationSettingsDto,
    ) -> Result<Self, Self::Error> {
        Ok(
            domain::notification::models::NotificationPreference::from_raw_parts(
                recipient.try_into()?,
                is_send_message_notification,
            ),
        )
    }
}

pub struct DiscordUserDto {
    pub user_id: String,
    pub username: String,
}

impl From<DiscordUserDto> for domain::user::models::DiscordUser {
    fn from(DiscordUserDto { user_id, username }: DiscordUserDto) -> Self {
        domain::user::models::DiscordUser::new(
            domain::user::models::DiscordUserId::new(user_id),
            domain::user::models::DiscordUserName::new(username),
        )
    }
}
