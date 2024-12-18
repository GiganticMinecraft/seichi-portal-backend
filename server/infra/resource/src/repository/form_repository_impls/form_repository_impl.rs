use async_trait::async_trait;
use domain::{
    form::models::{
        DefaultAnswerTitle, Form, FormDescription, FormId, FormTitle, ResponsePeriod, Visibility,
        WebhookUrl,
    },
    repository::form::form_repository::FormRepository,
    types::authorization_guard::{AuthorizationGuard, Create, Read},
    user::models::User,
};
use errors::Error;
use itertools::Itertools;
use outgoing::form_outgoing;

use crate::{
    database::components::{DatabaseComponents, FormDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> FormRepository for Repository<Client> {
    #[tracing::instrument(skip(self))]
    async fn create(&self, form: &Form, user: &User) -> Result<(), Error> {
        self.client
            .form()
            .create(form, user)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn list(
        &self,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Vec<AuthorizationGuard<Form, Read>>, Error> {
        self.client
            .form()
            .list(offset, limit)
            .await?
            .into_iter()
            .map(TryInto::<Form>::try_into)
            .map_ok(Into::<AuthorizationGuard<Form, Create>>::into)
            .map_ok(AuthorizationGuard::<_, Create>::into_read)
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn get(&self, id: FormId) -> Result<Option<AuthorizationGuard<Form, Read>>, Error> {
        Ok(self
            .client
            .form()
            .get(id)
            .await?
            .map(TryInto::<Form>::try_into)
            .transpose()?
            .map(Into::<AuthorizationGuard<Form, Create>>::into)
            .map(AuthorizationGuard::<_, Create>::into_read))
    }

    #[tracing::instrument(skip(self))]
    async fn delete(&self, id: FormId) -> Result<(), Error> {
        let form = self.client.form().get(id).await?;

        match form {
            None => Ok(()),
            Some(form) => {
                form_outgoing::delete(form.try_into()?).await?;
                self.client.form().delete(id).await.map_err(Into::into)
            }
        }
    }

    #[tracing::instrument(skip(self))]
    async fn update_title(&self, form_id: &FormId, title: &FormTitle) -> Result<(), Error> {
        self.client
            .form()
            .update_form_title(form_id, title)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn update_description(
        &self,
        form_id: &FormId,
        description: &FormDescription,
    ) -> Result<(), Error> {
        self.client
            .form()
            .update_form_description(form_id, description)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn update_response_period(
        &self,
        form_id: &FormId,
        response_period: &ResponsePeriod,
    ) -> Result<(), Error> {
        self.client
            .form()
            .update_form_response_period(form_id, response_period)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn update_webhook_url(
        &self,
        form_id: &FormId,
        webhook_url: &WebhookUrl,
    ) -> Result<(), Error> {
        self.client
            .form()
            .update_form_webhook_url(form_id, webhook_url)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn update_default_answer_title(
        &self,
        form_id: &FormId,
        default_answer_title: &DefaultAnswerTitle,
    ) -> Result<(), Error> {
        self.client
            .form()
            .update_form_default_answer_title(form_id, default_answer_title)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn update_visibility(
        &self,
        form_id: &FormId,
        visibility: &Visibility,
    ) -> Result<(), Error> {
        self.client
            .form()
            .update_form_visibility(form_id, visibility)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn update_answer_visibility(
        &self,
        form_id: &FormId,
        visibility: &Visibility,
    ) -> Result<(), Error> {
        self.client
            .form()
            .update_form_answer_visibility(form_id, visibility)
            .await
            .map_err(Into::into)
    }
}
