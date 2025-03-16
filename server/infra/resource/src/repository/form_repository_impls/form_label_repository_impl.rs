use async_trait::async_trait;
use domain::{
    form::models::{FormId, FormLabel, FormLabelId},
    repository::form::form_label_repository::FormLabelRepository,
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard_with_context::{Create, Delete, Read, Update},
    },
    user::models::User,
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
        actor: &User,
    ) -> Result<(), Error> {
        label
            .try_create(actor, |label| {
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
        actor: &User,
    ) -> Result<(), Error> {
        label
            .try_into_delete(actor, |label| {
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
        actor: &User,
    ) -> Result<(), Error> {
        label
            .try_update(actor, |label| {
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
    async fn replace_form_labels(
        &self,
        actor: &User,
        form_id: FormId,
        labels: Vec<AuthorizationGuard<FormLabel, Update>>,
    ) -> Result<(), Error> {
        let label_ids = labels
            .into_iter()
            .map(|guard| guard.try_into_update(actor, |label| label.id().to_owned()))
            .collect::<Result<Vec<_>, _>>()?;

        self.client
            .form_label()
            .replace_form_labels(form_id, label_ids)
            .await
            .map_err(Into::into)
    }
}
