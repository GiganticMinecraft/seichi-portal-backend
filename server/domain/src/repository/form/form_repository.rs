use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::form::answer::settings::models::{AnswerVisibility, DefaultAnswerTitle, ResponsePeriod};
use crate::{
    form::models::{Form, FormDescription, FormId, FormTitle, Visibility, WebhookUrl},
    types::authorization_guard::{AuthorizationGuard, Read},
    user::models::User,
};

#[automock]
#[async_trait]
pub trait FormRepository: Send + Sync + 'static {
    async fn create(&self, form: &Form, user: &User) -> Result<(), Error>;
    async fn list(
        &self,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Vec<AuthorizationGuard<Form, Read>>, Error>;
    async fn get(&self, id: FormId) -> Result<Option<AuthorizationGuard<Form, Read>>, Error>;
    async fn delete(&self, id: FormId) -> Result<(), Error>;
    async fn update_title(&self, form_id: &FormId, title: &FormTitle) -> Result<(), Error>;
    async fn update_description(
        &self,
        form_id: &FormId,
        description: &FormDescription,
    ) -> Result<(), Error>;
    async fn update_response_period(
        &self,
        form_id: &FormId,
        response_period: &ResponsePeriod,
    ) -> Result<(), Error>;
    async fn update_webhook_url(
        &self,
        form_id: &FormId,
        webhook_url: &WebhookUrl,
    ) -> Result<(), Error>;
    async fn update_default_answer_title(
        &self,
        form_id: &FormId,
        default_answer_title: &DefaultAnswerTitle,
    ) -> Result<(), Error>;
    async fn update_visibility(
        &self,
        form_id: &FormId,
        visibility: &Visibility,
    ) -> Result<(), Error>;
    async fn update_answer_visibility(
        &self,
        form_id: &FormId,
        visibility: &AnswerVisibility,
    ) -> Result<(), Error>;
}
