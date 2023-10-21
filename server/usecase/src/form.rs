use domain::{
    form::models::{
        Form, FormDescription, FormId, FormQuestionUpdateSchema, FormTitle, FormUpdateTargets,
        OffsetAndLimit, PostedAnswers,
    },
    repository::form_repository::FormRepository,
    user::models::User,
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
        user: User,
    ) -> Result<FormId, Error> {
        self.repository.create(title, description, user).await
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

    pub async fn get_all_answers(&self) -> Result<Vec<PostedAnswers>, Error> {
        self.repository.get_all_answers().await
    }

    pub async fn create_questions(&self, questions: FormQuestionUpdateSchema) -> Result<(), Error> {
        self.repository.create_questions(questions).await
    }
}
