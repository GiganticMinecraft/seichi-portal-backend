use domain::{
    form::models::{
        Form, FormDescription, FormId, FormTitle, FormUpdateTargets, OffsetAndLimit, PostedAnswers,
    },
    repository::form_repository::FormRepository,
};
use errors::Error;

pub struct FormUseCase<'a, FormRepo: FormRepository> {
    pub repository: &'a FormRepo,
}

impl<R: FormRepository> FormUseCase<'_, R> {
    pub async fn create_form(
        &self,
        title: FormTitle,
        description: FormDescription,
    ) -> Result<FormId, Error> {
        self.repository.create(title, description).await
    }

    pub async fn form_list(&self, offset_and_limit: OffsetAndLimit) -> Result<Vec<Form>, Error> {
        self.repository.list(offset_and_limit).await
    }

    pub async fn get_form(&self, form_id: FormId) -> Result<Form, Error> {
        self.repository.get(form_id).await
    }

    pub async fn delete_form(&self, form_id: FormId) -> Result<FormId, Error> {
        self.repository.delete(form_id).await
    }

    pub async fn update_form(
        &self,
        form_id: FormId,
        form_update_targets: FormUpdateTargets,
    ) -> Result<(), Error> {
        self.repository.update(form_id, form_update_targets).await
    }

    pub async fn post_answers(&self, answers: PostedAnswers) -> Result<(), Error> {
        self.repository.post_answer(answers).await
    }
}
