use async_trait::async_trait;
use domain::{
    form::{models::FormId, question::models::Question},
    repository::form::question_repository::QuestionRepository,
};
use errors::Error;

use crate::{
    database::components::{DatabaseComponents, FormQuestionDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> QuestionRepository for Repository<Client> {
    #[tracing::instrument(skip(self))]
    async fn create_questions(
        &self,
        form_id: FormId,
        questions: Vec<Question>,
    ) -> Result<(), Error> {
        self.client
            .form_question()
            .create_questions(form_id, questions)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn put_questions(&self, form_id: FormId, questions: Vec<Question>) -> Result<(), Error> {
        self.client
            .form_question()
            .put_questions(form_id, questions)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn get_questions(&self, form_id: FormId) -> Result<Vec<Question>, Error> {
        self.client
            .form_question()
            .get_questions(form_id)
            .await
            .map(|questions_dto| {
                questions_dto
                    .into_iter()
                    .map(|question_dto| question_dto.try_into())
                    .collect::<Result<Vec<Question>, _>>()
            })?
            .map_err(Into::into)
    }
}
