use async_trait::async_trait;
use domain::{
    form::models::{Form, FormId, FormTitle},
    repository::form_repository::FormRepository,
};

use crate::{
    database::components::{DatabaseComponents, FormDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> FormRepository for Repository<Client> {
    async fn create(&self, title: FormTitle) -> anyhow::Result<FormId> {
        self.client.form().create(title).await
    }

    async fn list(&self, offset: i32, limit: i32) -> anyhow::Result<Vec<Form>> {
        self.client.form().list(offset, limit).await
    }
}
