use async_trait::async_trait;
use domain::{
    form::models::{FormId, FormLabel, FormLabelId},
    repository::form::form_label_repository::FormLabelRepository,
};
use errors::Error;
use futures::{stream, StreamExt};

use crate::{
    database::components::{DatabaseComponents, FormLabelDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> FormLabelRepository for Repository<Client> {
    #[tracing::instrument(skip(self))]
    async fn create_label_for_forms(&self, label_name: String) -> Result<(), Error> {
        self.client
            .form_label()
            .create_label_for_forms(label_name)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn get_labels_for_forms(&self) -> Result<Vec<FormLabel>, Error> {
        stream::iter(self.client.form_label().get_labels_for_forms().await?)
            .then(|label_dto| async { Ok(label_dto.try_into()?) })
            .collect::<Vec<Result<FormLabel, _>>>()
            .await
            .into_iter()
            .collect::<Result<Vec<FormLabel>, _>>()
    }

    #[tracing::instrument(skip(self))]
    async fn delete_label_for_forms(&self, label_id: FormLabelId) -> Result<(), Error> {
        self.client
            .form_label()
            .delete_label_for_forms(label_id)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn edit_label_for_forms(&self, label: &FormLabel) -> Result<(), Error> {
        self.client
            .form_label()
            .edit_label_for_forms(label)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn replace_form_labels(
        &self,
        form_id: FormId,
        label_ids: Vec<FormLabelId>,
    ) -> Result<(), Error> {
        self.client
            .form_label()
            .replace_form_labels(form_id, label_ids)
            .await
            .map_err(Into::into)
    }
}
