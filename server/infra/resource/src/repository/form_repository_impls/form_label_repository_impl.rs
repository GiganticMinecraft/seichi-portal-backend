use async_trait::async_trait;
use domain::{
    form::models::{FormId, FormLabel, FormLabelId},
    repository::form::form_label_repository::FormLabelRepository,
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard_with_context::{Create, Delete, Read, Update},
    },
    user::models::{ActiveUser, User},
};
use errors::Error;
use itertools::Itertools;

use crate::{
    database::components::{DatabaseComponents, FormLabelDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> FormLabelRepository for Repository<Client> {
    #[tracing::instrument(skip(self))]
    async fn create_label_for_forms(
        &self,
        label: AuthorizationGuard<FormLabel, Create>,
        actor: &ActiveUser,
    ) -> Result<(), Error> {
        let actor_user = User::from(actor.clone());
        label
            .try_create(&actor_user, |label| {
                self.client.form_label().create_label_for_forms(label)
            })?
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn fetch_labels(&self) -> Result<Vec<AuthorizationGuard<FormLabel, Read>>, Error> {
        self.client
            .form_label()
            .fetch_labels()
            .await?
            .into_iter()
            .map(TryInto::<FormLabel>::try_into)
            .map_ok(Into::<AuthorizationGuard<_, Read>>::into)
            .collect::<Result<Vec<_>, _>>()
    }

    #[tracing::instrument(skip(self))]
    async fn fetch_labels_by_ids(
        &self,
        ids: Vec<FormLabelId>,
    ) -> Result<Vec<AuthorizationGuard<FormLabel, Read>>, Error> {
        self.client
            .form_label()
            .fetch_labels_by_ids(ids)
            .await?
            .into_iter()
            .map(TryInto::<FormLabel>::try_into)
            .map_ok(Into::<AuthorizationGuard<_, Read>>::into)
            .collect::<Result<Vec<_>, _>>()
    }

    #[tracing::instrument(skip(self))]
    async fn fetch_label(
        &self,
        id: FormLabelId,
    ) -> Result<Option<AuthorizationGuard<FormLabel, Read>>, Error> {
        Ok(self
            .client
            .form_label()
            .fetch_label(id)
            .await?
            .map(TryInto::<FormLabel>::try_into)
            .transpose()?
            .map(Into::into))
    }

    #[tracing::instrument(skip(self))]
    async fn delete_label_for_forms(
        &self,
        label: AuthorizationGuard<FormLabel, Delete>,
        actor: &ActiveUser,
    ) -> Result<(), Error> {
        let actor_user = User::from(actor.clone());
        label
            .try_into_delete(&actor_user, |label| {
                self.client
                    .form_label()
                    .delete_label_for_forms(label.id().to_owned())
            })?
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn edit_label_for_forms(
        &self,
        id: FormLabelId,
        label: AuthorizationGuard<FormLabel, Update>,
        actor: &ActiveUser,
    ) -> Result<(), Error> {
        let actor_user = User::from(actor.clone());
        label
            .try_update(&actor_user, |label| {
                self.client
                    .form_label()
                    .edit_label_for_forms(id, label.name().to_owned())
            })?
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn fetch_labels_by_form_id(
        &self,
        form_id: FormId,
    ) -> Result<Vec<AuthorizationGuard<FormLabel, Read>>, Error> {
        self.client
            .form_label()
            .fetch_labels_by_form_id(form_id)
            .await?
            .into_iter()
            .map(TryInto::<FormLabel>::try_into)
            .map_ok(Into::<AuthorizationGuard<_, Read>>::into)
            .collect::<Result<Vec<_>, _>>()
    }

    #[tracing::instrument(skip(self))]
    async fn size(&self) -> Result<u32, Error> {
        self.client.form_label().size().await.map_err(Into::into)
    }
}
