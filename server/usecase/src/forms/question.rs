use domain::{
    form::{models::FormId, question::models::Question},
    repository::form::question_repository::QuestionRepository,
    user::models::User,
};
use errors::Error;

pub struct QuestionUseCase<'a, QuestionRepo: QuestionRepository> {
    pub question_repository: &'a QuestionRepo,
}

impl<R1: QuestionRepository> QuestionUseCase<'_, R1> {
    pub async fn get_questions(
        &self,
        actor: &User,
        form_id: FormId,
    ) -> Result<Vec<Question>, Error> {
        self.question_repository
            .get_questions(form_id)
            .await?
            .into_iter()
            .map(|guard| guard.try_into_read(actor))
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub async fn create_questions(
        &self,
        actor: &User,
        form_id: FormId,
        questions: Vec<Question>,
    ) -> Result<(), Error> {
        self.question_repository
            .create_questions(
                actor,
                form_id,
                questions
                    .into_iter()
                    .map(|question| question.into())
                    .collect(),
            )
            .await
    }

    pub async fn put_questions(
        &self,
        actor: &User,
        form_id: FormId,
        questions: Vec<Question>,
    ) -> Result<Vec<Question>, Error> {
        self.question_repository
            .put_questions(
                actor,
                form_id,
                questions
                    .into_iter()
                    .map(|question| question.into())
                    .collect(),
            )
            .await?;

        self.question_repository
            .get_questions(form_id)
            .await?
            .into_iter()
            .map(|guard| guard.try_into_read(actor))
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}
