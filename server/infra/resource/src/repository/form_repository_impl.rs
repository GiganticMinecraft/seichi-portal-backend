use async_trait::async_trait;
use domain::{
    form::models::{FormId, FormTitle},
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
}
