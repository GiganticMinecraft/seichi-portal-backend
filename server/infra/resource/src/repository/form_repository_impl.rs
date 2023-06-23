use async_trait::async_trait;
use domain::{
    form::models::{Form, FormDescription, FormId, FormTitle},
    repository::form_repository::FormRepository,
};
use outgoing::form_outgoing;

use crate::{
    database::components::{DatabaseComponents, FormDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> FormRepository for Repository<Client> {
    async fn create(
        &self,
        title: FormTitle,
        description: FormDescription,
    ) -> anyhow::Result<FormId> {
        let form_id = self.client.form().create(title, description).await?;
        let form = self.client.form().get(form_id).await?;

        form_outgoing::create(form).await?;

        Ok(form_id)
    }

    async fn list(&self, offset: i32, limit: i32) -> anyhow::Result<Vec<Form>> {
        self.client.form().list(offset, limit).await
    }

    async fn get(&self, id: FormId) -> anyhow::Result<Form> {
        self.client.form().get(id).await
    }

    async fn delete(&self, id: FormId) -> anyhow::Result<FormId> {
        let form = self.client.form().get(id).await?;

        form_outgoing::delete(form).await?;

        self.client.form().delete(id).await
    }
}
