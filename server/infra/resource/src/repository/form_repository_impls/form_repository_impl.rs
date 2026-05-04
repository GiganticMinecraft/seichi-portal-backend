use async_trait::async_trait;
use domain::{
    form::models::{Form, FormId},
    repository::form::form_repository::FormRepository,
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard_with_context::{Create, Delete, Read, Update},
    },
    user::models::User,
};
use errors::Error;

use crate::{
    database::{
        components::{DatabaseComponents, FormDatabase},
        connection::DatabaseTransaction,
    },
    repository::Repository,
};

#[async_trait]
impl<Client> FormRepository for Repository<Client>
where
    Client: DatabaseComponents<TransactionAcrossComponents = DatabaseTransaction> + 'static,
{
    #[tracing::instrument(skip(self))]
    async fn create(
        &self,
        actor: &User,
        form: AuthorizationGuard<Form, Create>,
    ) -> Result<(), Error> {
        let form = form.try_into_create(actor, |form| form)?;
        self.client.form().create(&form, actor).await?;
        Ok(())
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
            .map(|form| form.map(|form| AuthorizationGuard::<Form, Create>::from(form).into_read()))
            .collect()
    }

    #[tracing::instrument(skip(self))]
    async fn get(&self, id: FormId) -> Result<Option<AuthorizationGuard<Form, Read>>, Error> {
        self.client
            .form()
            .get(id)
            .await?
            .map(TryInto::<Form>::try_into)
            .transpose()
            .map(|form| form.map(|form| AuthorizationGuard::<Form, Create>::from(form).into_read()))
    }

    #[tracing::instrument(skip(self))]
    async fn delete(
        &self,
        actor: &User,
        form: AuthorizationGuard<Form, Delete>,
    ) -> Result<(), Error> {
        form.try_delete(actor, |form| self.client.form().delete(*form.id()))?
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn update_form(
        &self,
        actor: &User,
        updated_form: AuthorizationGuard<Form, Update>,
    ) -> Result<(), Error> {
        let updated_form = updated_form.try_into_update(actor, |form| form)?;
        self.client.form().update(&updated_form, actor).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn size(&self) -> Result<u32, Error> {
        self.client.form().size().await.map_err(Into::into)
    }
}
