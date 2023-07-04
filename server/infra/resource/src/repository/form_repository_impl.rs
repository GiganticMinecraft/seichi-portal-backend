use async_trait::async_trait;
use domain::{
    form::models::{Form, FormDescription, FormId, FormTitle, FormUpdateTargets},
    repository::form_repository::FormRepository,
};
use errors::Error;
use outgoing::form_outgoing;

use crate::{
    database::components::{DatabaseComponents, FormDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> FormRepository for Repository<Client> {
    #[tracing::instrument(skip(self))]
    async fn create(
        &self,
        title: FormTitle,
        description: FormDescription,
    ) -> Result<FormId, Error> {
        let form_id = self.client.form().create(title, description).await?;
        let form = self.client.form().get(form_id.to_owned().into()).await?;

        form_outgoing::create(form.try_into()?).await?;

        Ok(form_id)
    }

    #[tracing::instrument(skip(self))]
    async fn list(&self, offset: i32, limit: i32) -> Result<Vec<Form>, Error> {
        let forms = self.client.form().list(offset, limit).await?;
        forms
            .into_iter()
            .map(|form| form.try_into())
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn get(&self, id: FormId) -> Result<Form, Error> {
        let form = self.client.form().get(id).await?;
        form.try_into().map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn delete(&self, id: FormId) -> Result<FormId, Error> {
        let form = self.client.form().get(id.to_owned().into()).await?;

        form_outgoing::delete(form.try_into()?).await?;

        self.client.form().delete(id).await.map_err(Into::into)
    }

    async fn update(
        &self,
        form_id: FormId,
        form_update_targets: FormUpdateTargets,
    ) -> Result<(), Error> {
        self.client
            .form()
            .update(form_id, form_update_targets)
            .await
            .map_err(Into::into)
    }
}
