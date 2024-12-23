use domain::{
    form::{models::FormId, question::models::Question},
    repository::form::question_repository::QuestionRepository,
};
use errors::Error;

pub struct QuestionUseCase<'a, QuestionRepo: QuestionRepository> {
    pub question_repository: &'a QuestionRepo,
}

impl<R1: QuestionRepository> QuestionUseCase<'_, R1> {
    pub async fn get_questions(&self, form_id: FormId) -> Result<Vec<Question>, Error> {
        self.question_repository.get_questions(form_id).await
    }

    pub async fn create_questions(
        &self,
        form_id: FormId,
        questions: Vec<Question>,
    ) -> Result<(), Error> {
        self.question_repository
            .create_questions(form_id, questions)
            .await
    }

    pub async fn put_questions(
        &self,
        form_id: FormId,
        questions: Vec<Question>,
    ) -> Result<(), Error> {
        self.question_repository
            .put_questions(form_id, questions)
            .await
    }
}
