use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::form::models::{FormId, FormLabel, FormLabelId};

#[automock]
#[async_trait]
pub trait FormLabelRepository: Send + Sync + 'static {
    async fn create_label_for_forms(&self, name: String) -> Result<(), Error>;
    async fn get_labels_for_forms(&self) -> Result<Vec<FormLabel>, Error>;
    async fn delete_label_for_forms(&self, label_id: FormLabelId) -> Result<(), Error>;
    async fn edit_label_for_forms(&self, label: &FormLabel) -> Result<(), Error>;
    async fn replace_form_labels(
        &self,
        form_id: FormId,
        label_ids: Vec<FormLabelId>,
    ) -> Result<(), Error>;
}
