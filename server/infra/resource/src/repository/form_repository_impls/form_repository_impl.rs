use async_trait::async_trait;
use domain::{
    account::models::AccountUser,
    form::models::{ActiveForm, ArchivedForm, ArchivedFormPagePosition, FormId, FormPagePosition},
    pagination::{Page, PageRequest},
    repository::form::{
        active_form_repository::ActiveFormRepository,
        archived_form_repository::ArchivedFormRepository,
    },
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Read, Update},
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
        actor: &AccountUser,
        form: Allowed<ActiveForm, Create>,
    ) -> Result<(), Error> {
        self.client.form().create(form.value(), actor).await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn list(
        &self,
        request: PageRequest<FormPagePosition>,
    ) -> Result<Page<AuthorizationGuard<ActiveForm, Read>, FormPagePosition>, Error> {
        let page = self.client.form().list(request).await?;
        let (forms, next) = page.into_parts();
        let forms = forms
            .into_iter()
            .map(TryInto::<ActiveForm>::try_into)
            .map(|form| {
                form.map(|form| AuthorizationGuard::<ActiveForm, Create>::from(form).into_read())
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Page::new(forms, next))
    }

    #[tracing::instrument(skip(self))]
    async fn list_all(&self) -> Result<Vec<AuthorizationGuard<ActiveForm, Read>>, Error> {
        self.client
            .form()
            .list_all()
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
        actor: &AccountUser,
        updated_form: Allowed<ActiveForm, Update>,
    ) -> Result<(), Error> {
        self.client
            .form()
            .update(updated_form.value(), actor)
            .await?;
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
        request: PageRequest<ArchivedFormPagePosition>,
        query: Option<String>,
    ) -> Result<Page<AuthorizationGuard<ArchivedForm, Read>, ArchivedFormPagePosition>, Error> {
        let page = self.client.form().list_archived(request, query).await?;
        let (forms, next) = page.into_parts();
        let forms = forms
            .into_iter()
            .map(TryInto::<ArchivedForm>::try_into)
            .map(|form| {
                form.map(|form| AuthorizationGuard::<ArchivedForm, Create>::from(form).into_read())
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Page::new(forms, next))
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
        form: Allowed<ArchivedForm, Create>,
    ) -> Result<AuthorizationGuard<ArchivedForm, Read>, Error> {
        let archived_form = self.client.form().archive(form.value()).await?;
        Ok(AuthorizationGuard::<ArchivedForm, Create>::from(archived_form).into_read())
    }

    #[tracing::instrument(skip(self))]
    async fn restore(&self, form: Allowed<ArchivedForm, Update>) -> Result<(), Error> {
        let form_id = *form.value().form().id();
        let _restored = form.into_inner().unarchive();
        self.client.form().restore(form_id).await?;
        Ok(())
    }
}
