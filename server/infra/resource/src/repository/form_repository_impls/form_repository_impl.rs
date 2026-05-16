use async_trait::async_trait;
use chrono::Utc;
use domain::{
    form::models::{ActiveForm, ArchivedForm, FormId},
    repository::form::{
        active_form_repository::ActiveFormRepository,
        archived_form_repository::ArchivedFormRepository,
    },
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard_with_context::{Create, Read, Update},
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
impl<Client> ActiveFormRepository for Repository<Client>
where
    Client: DatabaseComponents<TransactionAcrossComponents = DatabaseTransaction> + 'static,
{
    #[tracing::instrument(skip(self))]
    async fn create(
        &self,
        actor: &User,
        form: AuthorizationGuard<ActiveForm, Create>,
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
    ) -> Result<Vec<AuthorizationGuard<ActiveForm, Read>>, Error> {
        self.client
            .form()
            .list(offset, limit)
            .await?
            .into_iter()
            .map(TryInto::<ActiveForm>::try_into)
            .map(|form| {
                form.map(|form| AuthorizationGuard::<ActiveForm, Create>::from(form).into_read())
            })
            .collect()
    }

    #[tracing::instrument(skip(self))]
    async fn get(&self, id: FormId) -> Result<Option<AuthorizationGuard<ActiveForm, Read>>, Error> {
        self.client
            .form()
            .get(id)
            .await?
            .map(TryInto::<ActiveForm>::try_into)
            .transpose()
            .map(|form| {
                form.map(|form| AuthorizationGuard::<ActiveForm, Create>::from(form).into_read())
            })
    }

    #[tracing::instrument(skip(self))]
    async fn update_form(
        &self,
        actor: &User,
        updated_form: AuthorizationGuard<ActiveForm, Update>,
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

#[async_trait]
impl<Client> ArchivedFormRepository for Repository<Client>
where
    Client: DatabaseComponents<TransactionAcrossComponents = DatabaseTransaction> + 'static,
{
    #[tracing::instrument(skip(self))]
    async fn list(
        &self,
        offset: Option<u32>,
        limit: Option<u32>,
        query: Option<String>,
    ) -> Result<Vec<AuthorizationGuard<ArchivedForm, Read>>, Error> {
        self.client
            .form()
            .list_archived(offset, limit, query)
            .await?
            .into_iter()
            .map(TryInto::<ArchivedForm>::try_into)
            .map(|form| {
                form.map(|form| AuthorizationGuard::<ArchivedForm, Create>::from(form).into_read())
            })
            .collect()
    }

    #[tracing::instrument(skip(self))]
    async fn get(
        &self,
        id: FormId,
    ) -> Result<Option<AuthorizationGuard<ArchivedForm, Read>>, Error> {
        self.client
            .form()
            .get_archived(id)
            .await?
            .map(TryInto::<ArchivedForm>::try_into)
            .transpose()
            .map(|form| {
                form.map(|form| AuthorizationGuard::<ArchivedForm, Create>::from(form).into_read())
            })
    }

    #[tracing::instrument(skip(self))]
    async fn archive(
        &self,
        actor: &User,
        form: AuthorizationGuard<ActiveForm, Update>,
    ) -> Result<AuthorizationGuard<ArchivedForm, Read>, Error> {
        let form = form.try_into_update(actor, |form| form)?;
        let form_id = *form.id();
        let _ = form.archive(Utc::now(), actor.clone());
        let archived_form = self.client.form().archive(form_id, actor).await?;
        Ok(AuthorizationGuard::<ArchivedForm, Create>::from(archived_form).into_read())
    }

    #[tracing::instrument(skip(self))]
    async fn restore(
        &self,
        actor: &User,
        form: AuthorizationGuard<ArchivedForm, Update>,
    ) -> Result<(), Error> {
        let form = form.try_into_update(actor, |form| form)?;
        let form_id = *form.form().id();
        let _restored = form.restore();
        self.client.form().restore(form_id).await?;
        Ok(())
    }
}
