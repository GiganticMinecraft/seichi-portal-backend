use domain::{
    form::models::{FormId, FormLabel, FormLabelId, FormLabelIdSet, FormLabelName},
    repository::form::{
        active_form_repository::ActiveFormRepository, form_label_repository::FormLabelRepository,
    },
    user::models::User,
};
use errors::{Error, usecase::UseCaseError, usecase::UseCaseError::FormNotFound};

pub struct FormLabelUseCase<'a, FormLabelRepo: FormLabelRepository, FormRepo: ActiveFormRepository>
{
    pub form_label_repository: &'a FormLabelRepo,
    pub active_form_repository: &'a FormRepo,
}

impl<R1: FormLabelRepository, R2: ActiveFormRepository> FormLabelUseCase<'_, R1, R2> {
    pub async fn create_label_for_forms(
        &self,
        actor: &User,
        label_name: FormLabelName,
    ) -> Result<FormLabel, Error> {
        let label = FormLabel::new(label_name);
        let label_id = label.id().to_owned();

        self.form_label_repository
            .create_label_for_forms(label.into(), actor)
            .await?;

        self.form_label_repository
            .fetch_label(label_id)
            .await?
            .ok_or(Error::from(UseCaseError::LabelNotFound))?
            .try_into_read(actor)
            .map_err(Into::into)
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
        form_label_name: Option<FormLabelName>,
        actor: &User,
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

    pub async fn replace_form_labels(
        &self,
        actor: &User,
        form_id: FormId,
        label_ids: Vec<FormLabelId>,
    ) -> Result<(), Error> {
        let label_ids = FormLabelIdSet::try_new(label_ids)?;
        let updated_form = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?
            .into_update()
            .map(|form| form.replace_label_ids(label_ids.clone()));
        updated_form.try_update(actor, |_| ())?;

        let labels = self
            .form_label_repository
            .fetch_labels_by_ids(label_ids.as_slice().to_vec())
            .await?
            .into_iter()
            .map(|label| label.into_update())
            .collect::<Vec<_>>();
        if labels.len() != label_ids.as_slice().len() {
            return Err(Error::from(UseCaseError::LabelNotFound));
        }

        self.form_label_repository
            .replace_form_labels(actor, form_id, labels)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{
        form::{
            models::{
                ActiveForm, FormDescription, FormLabelIdSet, FormMeta, FormSettings, FormTitle,
                QuestionSet,
            },
            question::models::{Question, QuestionId, QuestionType},
        },
        repository::form::{
            active_form_repository::MockActiveFormRepository,
            form_label_repository::MockFormLabelRepository,
        },
        user::models::{Role, User},
    };
    use types::{non_empty_string::NonEmptyString, non_empty_vec::NonEmptyVec};
    use uuid::Uuid;

    fn admin_user() -> User {
        User {
            name: "admin".to_string(),
            id: Uuid::nil(),
            role: Role::Administrator,
        }
    }

    fn sample_form(form_id: FormId) -> ActiveForm {
        let questions = QuestionSet::try_new(
            NonEmptyVec::try_new(vec![
                Question::from_raw_parts(
                    QuestionId::from(Uuid::new_v4()),
                    "body".to_string().try_into().unwrap(),
                    0,
                    "Body".to_string().try_into().unwrap(),
                    None,
                    QuestionType::Text,
                    None,
                    true,
                )
                .unwrap(),
            ])
            .unwrap(),
        )
        .unwrap();

        ActiveForm::from_raw_parts(
            form_id,
            FormTitle::new("Form".to_string().try_into().unwrap()),
            FormDescription::new("description".to_string()),
            FormMeta::new(),
            FormSettings::new(),
            questions,
            FormLabelIdSet::empty(),
        )
    }

    #[tokio::test]
    async fn replace_form_labels_rejects_duplicate_label_ids() {
        let user = admin_user();
        let form_id = FormId::new();
        let label_id = FormLabelId::new();
        let form_label_repository = MockFormLabelRepository::new();
        let active_form_repository = MockActiveFormRepository::new();
        let usecase = FormLabelUseCase {
            form_label_repository: &form_label_repository,
            active_form_repository: &active_form_repository,
        };

        let result = usecase
            .replace_form_labels(&user, form_id, vec![label_id, label_id])
            .await;

        assert!(matches!(result, Err(Error::Domain { .. })));
    }

    #[tokio::test]
    async fn replace_form_labels_updates_after_form_domain_validation() {
        let user = admin_user();
        let form_id = FormId::new();
        let label_id = FormLabelId::new();

        let mut active_form_repository = MockActiveFormRepository::new();
        active_form_repository
            .expect_get()
            .times(1)
            .returning(move |_| Ok(Some(sample_form(form_id).into())));

        let mut form_label_repository = MockFormLabelRepository::new();
        form_label_repository
            .expect_fetch_labels_by_ids()
            .times(1)
            .returning(move |_| {
                Ok(vec![
                    FormLabel::from_raw_parts(
                        label_id,
                        FormLabelName::new(NonEmptyString::try_new("label".to_string()).unwrap()),
                    )
                    .into(),
                ])
            });
        form_label_repository
            .expect_replace_form_labels()
            .times(1)
            .returning(|_, _, _| Ok(()));

        let usecase = FormLabelUseCase {
            form_label_repository: &form_label_repository,
            active_form_repository: &active_form_repository,
        };

        usecase
            .replace_form_labels(&user, form_id, vec![label_id])
            .await
            .unwrap();
    }
}
