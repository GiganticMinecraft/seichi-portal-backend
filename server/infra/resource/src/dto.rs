use chrono::{DateTime, Utc};
use domain::form::models::{FormSettings, ResponsePeriod};
use uuid::Uuid;

#[derive(Clone)]
pub struct QuestionDto {
    pub id: i32,
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
            .id(id.into())
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
            })
            .build())
    }
}

pub struct SimpleFormDto {
    pub id: i32,
    pub title: String,
    pub description: Option<String>,
    pub response_period: Option<(DateTime<Utc>, DateTime<Utc>)>,
}

impl TryFrom<SimpleFormDto> for domain::form::models::SimpleForm {
    type Error = errors::domain::DomainError;

    fn try_from(
        SimpleFormDto {
            id,
            title,
            description,
            response_period,
        }: SimpleFormDto,
    ) -> Result<Self, Self::Error> {
        Ok(domain::form::models::SimpleForm {
            id: id.into(),
            title: title.into(),
            description: description.into(),
            response_period: ResponsePeriod::new(response_period),
        })
    }
}

pub struct AnswerDto {
    pub question_id: i32,
    pub answer: String,
}

impl TryFrom<AnswerDto> for domain::form::models::Answer {
    type Error = errors::domain::DomainError;

    fn try_from(
        AnswerDto {
            question_id,
            answer,
        }: AnswerDto,
    ) -> Result<Self, Self::Error> {
        Ok(domain::form::models::Answer {
            question_id: question_id.into(),
            answer,
        })
    }
}

pub struct PostedAnswersDto {
    pub uuid: Uuid,
    pub timestamp: DateTime<Utc>,
    pub form_id: i32,
    pub title: Option<String>,
    pub answers: Vec<AnswerDto>,
}

impl TryFrom<PostedAnswersDto> for domain::form::models::PostedAnswers {
    type Error = errors::domain::DomainError;

    fn try_from(
        PostedAnswersDto {
            uuid,
            timestamp,
            form_id,
            title,
            answers,
        }: PostedAnswersDto,
    ) -> Result<Self, Self::Error> {
        Ok(domain::form::models::PostedAnswers {
            id: todo!(),
            uuid,
            timestamp,
            form_id: form_id.into(),
            title: title.into(),
            answers: answers
                .into_iter()
                .map(|answer| answer.try_into())
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}
