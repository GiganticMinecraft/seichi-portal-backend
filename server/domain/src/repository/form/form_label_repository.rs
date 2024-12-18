use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::form::models::{FormId, Label, LabelId};

#[automock]
#[async_trait]
pub trait FormLabelRepository: Send + Sync + 'static {
    async fn create_label_for_forms(&self, name: String) -> Result<(), Error>;
    async fn get_labels_for_forms(&self) -> Result<Vec<Label>, Error>;
    async fn delete_label_for_forms(&self, label_id: LabelId) -> Result<(), Error>;
    async fn edit_label_for_forms(&self, label: &Label) -> Result<(), Error>;
    async fn replace_form_labels(
        &self,
        form_id: FormId,
        label_ids: Vec<LabelId>,
    ) -> Result<(), Error>;
}
