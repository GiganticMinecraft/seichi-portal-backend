use std::{collections::HashMap, str::FromStr};

use async_trait::async_trait;
use domain::{
    form::models::{FormId, FormLabel, FormLabelId},
    repository::form::form_label_repository::FormLabelRepository,
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Delete, Read, Update},
};
use errors::{Error, infra::InfraError};
use itertools::Itertools;
use uuid::Uuid;

use crate::{
    database::components::{DatabaseComponents, FormLabelDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> FormLabelRepository for Repository<Client> {
    #[tracing::instrument(skip_all)]
    async fn create_label_for_forms(&self, label: Allowed<FormLabel, Create>) -> Result<(), Error> {
        self.client
            .form_label()
            .create_label_for_forms(label.value())
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip_all)]
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

    #[tracing::instrument(skip_all)]
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

    #[tracing::instrument(skip_all, fields(label_id = %id))]
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

    #[tracing::instrument(skip_all)]
    async fn delete_label_for_forms(&self, label: Allowed<FormLabel, Delete>) -> Result<(), Error> {
        self.client
            .form_label()
            .delete_label_for_forms(label.value().id().to_owned())
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip_all, fields(label_id = %id))]
    async fn edit_label_for_forms(
        &self,
        id: FormLabelId,
        label: Allowed<FormLabel, Update>,
    ) -> Result<(), Error> {
        self.client
            .form_label()
            .edit_label_for_forms(id, label.value().name().to_owned())
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip_all, fields(form_id = %form_id))]
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

    #[tracing::instrument(skip_all, fields(form_count = form_ids.len()))]
    async fn fetch_labels_by_form_ids(
        &self,
        form_ids: Vec<FormId>,
    ) -> Result<HashMap<FormId, Vec<AuthorizationGuard<FormLabel, Read>>>, Error> {
        let mut labels_by_form = form_ids
            .iter()
            .map(|form_id| (*form_id, Vec::new()))
            .collect::<HashMap<_, _>>();

        for (form_id, record) in self
            .client
            .form_label()
            .fetch_labels_by_form_ids(form_ids)
            .await?
        {
            let form_id = FormId::from(Uuid::from_str(&form_id).map_err(Into::<InfraError>::into)?);
            let label = FormLabel::try_from(record)?;
            labels_by_form
                .entry(form_id)
                .or_default()
                .push(AuthorizationGuard::<_, Read>::from(label));
        }

        Ok(labels_by_form)
    }

    #[tracing::instrument(skip_all)]
    async fn size(&self) -> Result<u32, Error> {
        self.client.form_label().size().await.map_err(Into::into)
    }
}
