use domain::{
    form::models::{FormLabel, FormLabelId, FormLabelName},
    repository::form::form_label_repository::FormLabelRepository,
    user::models::{ActiveUser, User},
};
use errors::{Error, usecase::UseCaseError};

pub struct FormLabelUseCase<'a, FormLabelRepo: FormLabelRepository> {
    pub form_label_repository: &'a FormLabelRepo,
}

impl<R: FormLabelRepository> FormLabelUseCase<'_, R> {
    pub async fn create_label_for_forms(
        &self,
        actor: &ActiveUser,
        label_name: FormLabelName,
    ) -> Result<FormLabel, Error> {
        let actor_user = User::from(actor.clone());
        let label = FormLabel::new(label_name);
        let label_id = label.id().to_owned();

        self.form_label_repository
            .create_label_for_forms(label.into(), actor)
            .await?;

        self.form_label_repository
            .fetch_label(label_id)
            .await?
            .ok_or(Error::from(UseCaseError::LabelNotFound))?
            .try_into_read(&actor_user)
            .map_err(Into::into)
    }

    pub async fn get_labels_for_forms(&self, actor: &ActiveUser) -> Result<Vec<FormLabel>, Error> {
        let actor_user = User::from(actor.clone());
        self.form_label_repository
            .fetch_labels()
            .await?
            .into_iter()
            .map(|label| label.try_into_read(&actor_user))
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub async fn delete_label_for_forms(
        &self,
        label_id: FormLabelId,
        actor: &ActiveUser,
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
        form_label_name: Option<FormLabelName>,
        actor: &ActiveUser,
    ) -> Result<(), Error> {
        let current_label = self
            .form_label_repository
            .fetch_label(id.to_owned())
            .await?
            .ok_or(UseCaseError::LabelNotFound)?;

        if let Some(name) = form_label_name {
            let renamed_label = current_label.into_update().map(|label| label.renamed(name));

            self.form_label_repository
                .edit_label_for_forms(id, renamed_label, actor)
                .await?;
        }

        Ok(())
    }
}
