use std::str::FromStr;

use chrono::{DateTime, Utc};
use domain::{
    form::{
        answer::{
            models::{AnswerEntry, AnswerLabel, AnswerTitle, FormAnswerContent},
            settings::models::{DefaultAnswerTitle, ResponsePeriod},
        },
        comment::models::CommentContent,
        models::{
            ActiveForm, ArchivedForm, FormDescription, FormId, FormLabel, FormLabelId,
            FormLabelIdSet, FormLabelName, FormMeta, FormSettings, FormTitle, QuestionSet,
            WebhookUrl,
        },
        question::models::{Choice, Question, QuestionType},
    },
    user::models::{Role, User},
};
use errors::infra::InfraError;
use types::non_empty_string::NonEmptyString;
use types::non_empty_vec::NonEmptyVec;
use uuid::Uuid;

#[derive(Clone)]
pub struct ChoiceDto {
    pub id: Option<i32>,
    pub position: u16,
    pub label: String,
}

impl TryFrom<ChoiceDto> for Choice {
    type Error = errors::Error;

    fn try_from(
        ChoiceDto {
            id,
            position,
            label,
        }: ChoiceDto,
    ) -> Result<Self, Self::Error> {
        Choice::from_raw_parts(id.map(Into::into), position, label.try_into()?).map_err(Into::into)
    }
}

#[derive(Clone)]
pub struct QuestionDto {
    pub id: String,
    pub form_id: String,
    pub template_key: String,
    pub position: u16,
    pub title: String,
    pub description: Option<String>,
    pub question_type: String,
    pub choices: Vec<ChoiceDto>,
    pub is_required: bool,
}

impl TryFrom<QuestionDto> for Question {
    type Error = errors::Error;

    fn try_from(
        QuestionDto {
            id,
            form_id: _,
            template_key,
            position,
            title,
            description,
            question_type,
            choices,
            is_required,
        }: QuestionDto,
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

pub struct ActiveFormDto {
    pub id: String,
    pub title: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
    pub webhook_url: Option<String>,
    pub default_answer_title: Option<String>,
    pub visibility: String,
    pub answer_visibility: String,
    pub questions: Vec<QuestionDto>,
    pub label_ids: Vec<FormLabelId>,
}

pub type FormDto = ActiveFormDto;

impl TryFrom<ActiveFormDto> for ActiveForm {
    type Error = errors::Error;

    fn try_from(
        ActiveFormDto {
            id,
            title,
            description,
            created_at,
            updated_at,
            start_at,
            end_at,
            webhook_url,
            default_answer_title,
            visibility,
            answer_visibility,
            questions,
            label_ids,
        }: ActiveFormDto,
    ) -> Result<Self, Self::Error> {
        let questions = questions
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, _>>()?;
        let questions = NonEmptyVec::try_new(questions).map_err(errors::Error::from)?;

        Ok(ActiveForm::from_raw_parts(
            FormId::from(Uuid::parse_str(&id).map_err(Into::<InfraError>::into)?),
            FormTitle::new(title.try_into()?),
            FormDescription::new(description),
            FormMeta::from_raw_parts(created_at, updated_at),
            FormSettings::from_raw_parts(
                ResponsePeriod::try_new(start_at, end_at)?,
                WebhookUrl::try_new(webhook_url.map(NonEmptyString::try_new).transpose()?)?,
                DefaultAnswerTitle::new(
                    default_answer_title
                        .map(NonEmptyString::try_new)
                        .transpose()?,
                ),
                visibility.try_into()?,
                answer_visibility.try_into()?,
            ),
            QuestionSet::try_new(questions).map_err(errors::Error::from)?,
            FormLabelIdSet::try_new(label_ids)?,
        ))
    }
}

pub struct ArchivedFormDto {
    pub form: ActiveFormDto,
    pub archived_at: DateTime<Utc>,
    pub archived_by_name: String,
    pub archived_by_id: String,
    pub archived_by_role: Role,
}

impl TryFrom<ArchivedFormDto> for ArchivedForm {
    type Error = errors::Error;

    fn try_from(value: ArchivedFormDto) -> Result<Self, Self::Error> {
        Ok(ArchivedForm::from_persisted(
            value.form.try_into()?,
            value.archived_at,
            UserDto {
                name: value.archived_by_name,
                id: value.archived_by_id,
                role: value.archived_by_role,
            }
            .try_into()?,
        ))
    }
}

#[derive(Clone)]
pub struct FormAnswerContentDto {
    pub id: String,
    pub question_id: String,
    pub answer: String,
}

impl TryFrom<FormAnswerContentDto> for FormAnswerContent {
    type Error = InfraError;

    fn try_from(
        FormAnswerContentDto {
            id,
            question_id,
            answer,
        }: FormAnswerContentDto,
    ) -> Result<Self, Self::Error> {
        Ok(FormAnswerContent {
            id: Uuid::parse_str(&id)?.into(),
            question_id: Uuid::parse_str(&question_id)?.into(),
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
    pub commented_by_name: String,
    pub commented_by_id: String,
    pub commented_by_role: String,
}

impl TryFrom<CommentDto> for domain::form::comment::models::Comment {
    type Error = errors::Error;

    fn try_from(
        CommentDto {
            answer_id,
            comment_id,
            content,
            timestamp,
            commented_by_name,
            commented_by_id,
            commented_by_role,
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
            UserDto {
                name: commented_by_name,
                id: commented_by_id,
                role: Role::from_str(&commented_by_role).map_err(Into::<InfraError>::into)?,
            }
            .try_into()?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn question_dto_rejects_text_question_with_choices() {
        let result: Result<Question, _> = QuestionDto {
            id: Uuid::nil().to_string(),
            form_id: Uuid::nil().to_string(),
            template_key: "template".to_string(),
            position: 0,
            title: "Question".to_string(),
            description: None,
            question_type: "Text".to_string(),
            choices: vec![ChoiceDto {
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

impl TryFrom<FormAnswerDto> for AnswerEntry {
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
            Ok(AnswerEntry::from_raw_parts(
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

impl TryFrom<AnswerLabelDto> for AnswerLabel {
    type Error = errors::Error;

    fn try_from(AnswerLabelDto { id, name }: AnswerLabelDto) -> Result<Self, Self::Error> {
        Ok(AnswerLabel::from_raw_parts(
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

impl TryFrom<FormLabelDto> for FormLabel {
    type Error = errors::Error;

    fn try_from(FormLabelDto { id, name }: FormLabelDto) -> Result<Self, Self::Error> {
        Ok(FormLabel::from_raw_parts(
            Uuid::from_str(&id)
                .map_err(Into::<InfraError>::into)?
                .into(),
            FormLabelName::new(name.try_into()?),
        ))
    }
}

pub struct MessageDto {
    pub id: String,
    pub related_answer: String,
    pub sender_name: String,
    pub sender_id: String,
    pub sender_role: String,
    pub body: String,
    pub timestamp: DateTime<Utc>,
}

impl TryFrom<MessageDto> for domain::form::message::models::Message {
    type Error = errors::Error;

    fn try_from(
        MessageDto {
            id,
            related_answer,
            sender_name,
            sender_id,
            sender_role,
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
                UserDto {
                    name: sender_name,
                    id: sender_id,
                    role: Role::from_str(&sender_role).map_err(Into::<InfraError>::into)?,
                }
                .try_into()?,
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
