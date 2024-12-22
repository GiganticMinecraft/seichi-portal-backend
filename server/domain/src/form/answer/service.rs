use errors::{domain::DomainError, Error};

use crate::{
    form::{
        answer::models::{FormAnswer, FormAnswerContent},
        models::Visibility,
    },
    repository::form::{answer_repository::AnswerRepository, form_repository::FormRepository},
};

pub struct AnswerService<'a, AnswerRepo: AnswerRepository, FormRepo: FormRepository> {
    pub answer_repo: &'a AnswerRepo,
    pub form_repo: &'a FormRepo,
}

impl<R1: AnswerRepository, R2: FormRepository> AnswerService<'_, R1, R2> {
    pub async fn post_answer(
        &self,
        answer: FormAnswer,
        answer_contents: Vec<FormAnswerContent>,
    ) -> Result<(), Error> {
        let form = self
            .form_repo
            .get(*answer.form_id())
            .await?
            .ok_or(DomainError::NotFound)?
            .try_into_read(answer.user())?;

        let form_settings = form.settings();

        if *form_settings.visibility() == Visibility::PRIVATE {
            return Err(Error::from(DomainError::Forbidden));
        }

        let start_at = *form_settings.response_period().start_at();
        let end_at = *form_settings.response_period().end_at();

        if let Some(start_at) = start_at {
            if start_at > chrono::Utc::now() {
                return Err(Error::from(DomainError::Forbidden));
            }
        }

        if let Some(end_at) = end_at {
            if end_at < chrono::Utc::now() {
                return Err(Error::from(DomainError::Forbidden));
            }
        }

        self.answer_repo.post_answer(&answer, answer_contents).await
    }
}
