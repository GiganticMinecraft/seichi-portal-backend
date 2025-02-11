use async_trait::async_trait;
use domain::{
    form::models::{Form, FormId},
    repository::form::form_repository::FormRepository,
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard_with_context::{Create, Read, Update},
    },
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
    async fn update_form(
        &self,
        actor: &User,
        updated_form: AuthorizationGuard<Form, Update>,
    ) -> Result<(), Error> {
        updated_form
            .try_update(actor, |form| self.client.form().update(form, actor))?
            .await
            .map_err(Into::into)
    }
}
