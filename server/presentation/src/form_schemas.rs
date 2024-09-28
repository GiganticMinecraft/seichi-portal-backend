use domain::form::models::{
    DefaultAnswerTitle, FormDescription, FormTitle, ResponsePeriod, Visibility, WebhookUrl,
};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct CreateFormSchema {
    pub title: FormTitle,
    pub description: FormDescription,
}

#[derive(Deserialize, Debug)]
pub struct UpdateFormSchema {
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
