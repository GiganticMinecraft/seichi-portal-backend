use chrono::{DateTime, Utc};
use domain::form::models::{FormSettings, ResponsePeriod};

pub struct QuestionDto {
    pub id: i32,
    pub title: String,
    pub description: Option<String>,
    pub question_type: String,
    pub choices: Vec<String>,
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
        }: QuestionDto,
    ) -> Result<Self, Self::Error> {
        Ok(domain::form::models::Question::builder()
            .id(id.into())
            .title(title)
            .description(description)
            .question_type(question_type.try_into()?)
            .choices(choices)
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
            })
            .build())
    }
}
