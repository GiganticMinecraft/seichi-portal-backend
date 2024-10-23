use chrono::{DateTime, Utc};
use domain::{
    form::models::{FormSettings, ResponsePeriod},
    user::models::{Role, User},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct QuestionDto {
    pub id: Option<i32>,
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
            title,
            description,
            question_type,
            choices,
            is_required,
        }: QuestionDto,
    ) -> Result<Self, Self::Error> {
        Ok(domain::form::models::Question::builder()
            .id(id.map(Into::into))
            .title(title)
            .description(description)
            .question_type(question_type.try_into()?)
            .choices(choices)
            .is_required(is_required)
            .build())
    }
}

pub struct FormDto {
    pub id: i32,
    pub title: String,
    pub description: Option<String>,
    pub questions: Vec<QuestionDto>,
    pub metadata: (DateTime<Utc>, DateTime<Utc>),
    pub response_period: Option<(DateTime<Utc>, DateTime<Utc>)>,
    pub webhook_url: Option<String>,
    pub default_answer_title: Option<String>,
    pub visibility: String,
    pub labels: Vec<LabelDto>,
    pub answer_visibility: String,
}

impl TryFrom<FormDto> for domain::form::models::Form {
    type Error = errors::domain::DomainError;

    fn try_from(
        FormDto {
            id,
            title,
            description,
            questions,
            metadata,
            response_period,
            webhook_url,
            default_answer_title,
            visibility,
            labels,
            answer_visibility,
        }: FormDto,
    ) -> Result<Self, Self::Error> {
        Ok(domain::form::models::Form::builder()
            .id(id)
            .title(title)
            .description(description)
            .questions(
                questions
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<Vec<_>, _>>()?,
            )
            .metadata(metadata)
            .settings(FormSettings {
                response_period: ResponsePeriod::new(response_period),
                webhook_url: webhook_url.into(),
                default_answer_title: default_answer_title.into(),
                visibility: visibility.try_into()?,
                answer_visibility: answer_visibility.try_into()?,
            })
            .labels(
                labels
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<Vec<_>, _>>()?,
            )
            .build())
    }
}

pub struct SimpleFormDto {
    pub id: i32,
    pub title: String,
    pub description: Option<String>,
    pub response_period: Option<(DateTime<Utc>, DateTime<Utc>)>,
    pub labels: Vec<LabelDto>,
    pub answer_visibility: String,
}

impl TryFrom<SimpleFormDto> for domain::form::models::SimpleForm {
    type Error = errors::domain::DomainError;

    fn try_from(
        SimpleFormDto {
            id,
            title,
            description,
            response_period,
            labels,
            answer_visibility,
        }: SimpleFormDto,
    ) -> Result<Self, Self::Error> {
        Ok(domain::form::models::SimpleForm {
            id: id.into(),
            title: title.into(),
            description: description.into(),
            response_period: ResponsePeriod::new(response_period),
            labels: labels
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
            answer_visibility: answer_visibility.try_into()?,
        })
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
    pub form_id: i32,
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
            form_id: form_id.into(),
            title: title.into(),
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
