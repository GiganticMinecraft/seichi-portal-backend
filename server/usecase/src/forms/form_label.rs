use domain::{
    form::models::{FormId, Label, LabelId},
    repository::form::form_label_repository::FormLabelRepository,
};
use errors::Error;

pub struct FormLabelUseCase<'a, FormLabelRepo: FormLabelRepository> {
    pub form_label_repository: &'a FormLabelRepo,
}

impl<R1: FormLabelRepository> FormLabelUseCase<'_, R1> {
    pub async fn create_label_for_forms(&self, label_name: String) -> Result<(), Error> {
        self.form_label_repository
            .create_label_for_forms(label_name)
            .await
    }

    pub async fn get_labels_for_forms(&self) -> Result<Vec<Label>, Error> {
        self.form_label_repository.get_labels_for_forms().await
    }

    pub async fn delete_label_for_forms(&self, label_id: LabelId) -> Result<(), Error> {
        self.form_label_repository
            .delete_label_for_forms(label_id)
            .await
    }

    pub async fn edit_label_for_forms(&self, label: &Label) -> Result<(), Error> {
        self.form_label_repository.edit_label_for_forms(label).await
    }

    pub async fn replace_form_labels(
        &self,
        form_id: FormId,
        label_ids: Vec<LabelId>,
    ) -> Result<(), Error> {
        self.form_label_repository
            .replace_form_labels(form_id, label_ids)
            .await
    }
}
