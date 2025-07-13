use domain::form::question::models::{QuestionId, QuestionType};
use domain::form::{
    answer::{
        models::{AnswerId, AnswerLabelId, AnswerTitle, FormAnswerContent},
        settings::models::{AnswerVisibility, DefaultAnswerTitle, ResponsePeriod},
    },
    models::{FormDescription, FormId, FormLabelId, FormTitle, Visibility, WebhookUrl},
};
use serde::Deserialize;
use types::non_empty_string::NonEmptyString;

#[derive(Deserialize, Debug)]
pub struct OffsetAndLimit {
    pub offset: Option<u32>,
    pub limit: Option<u32>,
}

#[derive(Deserialize, Debug)]
pub struct FormCreateSchema {
    pub title: FormTitle,
    pub description: FormDescription,
}

#[derive(Deserialize, Debug)]
pub struct FormUpdateSchema {
    #[serde(default)]
    pub title: Option<FormTitle>,
    #[serde(default)]
    pub description: Option<FormDescription>,
    #[serde(default)]
    pub response_period: Option<ResponsePeriod>,
    #[serde(default)]
    pub webhook: Option<WebhookUrl>,
    #[serde(default)]
    pub default_answer_title: Option<DefaultAnswerTitle>,
    #[serde(default)]
    pub visibility: Option<Visibility>,
    #[serde(default)]
    pub answer_visibility: Option<AnswerVisibility>,
}

#[derive(Deserialize, Debug)]
pub struct AnswersPostSchema {
    pub form_id: FormId,
    pub title: DefaultAnswerTitle,
    pub answers: Vec<FormAnswerContent>,
}

#[derive(Deserialize, Debug)]
pub struct AnswerUpdateSchema {
    #[serde(default)]
    pub title: Option<AnswerTitle>,
}

#[derive(Deserialize, Debug)]
pub struct QuestionSchema {
    pub id: Option<QuestionId>,
    pub title: String,
    pub question_type: QuestionType,
    pub description: Option<String>,
    #[serde(default)]
    pub choices: Vec<String>,
    pub is_required: bool,
}

#[derive(Deserialize, Debug)]
pub struct FormQuestionPutSchema {
    #[serde(default)]
    pub questions: Vec<QuestionSchema>,
}

#[derive(Deserialize, Debug)]
pub struct CommentPostSchema {
    pub answer_id: AnswerId,
    pub content: String,
}

#[derive(Deserialize, Debug)]
pub struct FormLabelCreateSchema {
    pub name: NonEmptyString,
}

#[derive(Deserialize, Debug)]
pub struct FormLabelUpdateSchema {
    pub name: Option<NonEmptyString>,
}

#[derive(Deserialize, Debug)]
pub struct AnswerLabelSchema {
    pub name: NonEmptyString,
}

#[derive(Deserialize, Debug)]
pub struct AnswerLabelUpdateSchema {
    pub name: Option<NonEmptyString>,
}

#[derive(Deserialize, Debug)]
pub struct ReplaceAnswerLabelSchema {
    pub labels: Vec<AnswerLabelId>,
}

#[derive(Deserialize, Debug)]
pub struct ReplaceFormLabelSchema {
    pub labels: Vec<FormLabelId>,
}

#[derive(Deserialize, Debug)]
pub struct PostedMessageSchema {
    pub body: String,
}

#[derive(Deserialize, Debug)]
pub struct MessageUpdateSchema {
    pub body: String,
}
