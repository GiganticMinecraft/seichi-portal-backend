use domain::form::models::{
    Answer, DefaultAnswerTitle, FormDescription, FormId, FormTitle, ResponsePeriod, Visibility,
    WebhookUrl,
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
    pub answers: Vec<Answer>,
}

#[derive(Deserialize, Debug)]
pub struct AnswerUpdateSchema {
    #[serde(default)]
    pub title: Option<String>,
}
