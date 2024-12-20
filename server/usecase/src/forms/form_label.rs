use domain::{
    form::models::{FormId, FormLabel, FormLabelId, FormLabelName},
    repository::form::form_label_repository::FormLabelRepository,
    user::models::User,
};
use errors::{usecase::UseCaseError, Error};

pub struct FormLabelUseCase<'a, FormLabelRepo: FormLabelRepository> {
    pub form_label_repository: &'a FormLabelRepo,
}

impl<R1: FormLabelRepository> FormLabelUseCase<'_, R1> {
    pub async fn create_label_for_forms(
        &self,
        actor: &User,
        label_name: FormLabelName,
    ) -> Result<(), Error> {
        self.form_label_repository
            .create_label_for_forms(FormLabel::new(label_name).into(), actor)
            .await
    }

    pub async fn get_labels_for_forms(&self, actor: &User) -> Result<Vec<FormLabel>, Error> {
        self.form_label_repository
            .fetch_labels()
            .await?
            .into_iter()
            .map(|label| label.try_into_read(actor))
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub async fn delete_label_for_forms(
        &self,
        label_id: FormLabelId,
        actor: &User,
    ) -> Result<(), Error> {
        let label = self
            .form_label_repository
            .fetch_label(label_id)
            .await?
            .ok_or(UseCaseError::LabelNotFound)?
            .into_delete();

        self.form_label_repository
            .delete_label_for_forms(label, actor)
            .await
    }

    pub async fn edit_label_for_forms(
        &self,
        id: FormLabelId,
        form_label_name: FormLabelName,
        actor: &User,
    ) -> Result<(), Error> {
        let current_label = self
            .form_label_repository
            .fetch_label(id.to_owned())
            .await?
            .ok_or(UseCaseError::LabelNotFound)?;

        let renamed_label = current_label
            .into_update()
            .map(actor, |label| label.renamed(form_label_name));

        self.form_label_repository
            .edit_label_for_forms(id, renamed_label, actor)
            .await
    }

    pub async fn replace_form_labels(
        &self,
        form_id: FormId,
        label_ids: Vec<FormLabelId>,
    ) -> Result<(), Error> {
        self.form_label_repository
            .replace_form_labels(form_id, label_ids)
            .await
    }
}
