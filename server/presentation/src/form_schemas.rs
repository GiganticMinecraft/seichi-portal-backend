use domain::form::models::{
    AnswerContent, AnswerId, DefaultAnswerTitle, FormDescription, FormId, FormTitle, LabelId,
    Question, ResponsePeriod, Visibility, WebhookUrl,
};
use serde::Deserialize;

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
    pub has_response_period: Option<bool>,
    #[serde(default)]
    pub response_period: Option<ResponsePeriod>,
    #[serde(default)]
    pub webhook: Option<WebhookUrl>,
    #[serde(default)]
    pub default_answer_title: Option<DefaultAnswerTitle>,
    #[serde(default)]
    pub visibility: Option<Visibility>,
    #[serde(default)]
    pub answer_visibility: Option<Visibility>,
}

#[derive(Deserialize, Debug)]
pub struct AnswersPostSchema {
    pub form_id: FormId,
    pub title: DefaultAnswerTitle,
    pub answers: Vec<AnswerContent>,
}

#[derive(Deserialize, Debug)]
pub struct AnswerUpdateSchema {
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct FormQuestionUpdateSchema {
    pub form_id: FormId,
    pub questions: Vec<Question>,
}

#[derive(Deserialize, Debug)]
pub struct CommentPostSchema {
    pub answer_id: AnswerId,
    pub content: String,
}

#[derive(Deserialize, Debug)]
pub struct LabelSchema {
    pub name: String,
}

#[derive(Deserialize, Debug)]
pub struct ReplaceAnswerLabelSchema {
    pub labels: Vec<LabelId>,
}
