use chrono::Utc;
use errors::{domain::DomainError, Error};

use crate::{
    form::{
        answer::models::{AnswerTitle, FormAnswer},
        models::{FormId, Visibility},
    },
    repository::form::form_repository::FormRepository,
    user::models::User,
};

pub struct AnswerService<'a, FormRepo: FormRepository> {
    pub form_repo: &'a FormRepo,
}

impl<R1: FormRepository> AnswerService<'_, R1> {
    pub async fn new_form_answer(
        &self,
        user: User,
        form_id: FormId,
        title: AnswerTitle,
    ) -> Result<FormAnswer, Error> {
        let form = self
            .form_repo
            .get(form_id)
            .await?
            .ok_or(DomainError::NotFound)?
            .try_into_read(&user)?;

        let form_settings = form.settings();

        let is_public_form = *form_settings.visibility() == Visibility::PUBLIC;
        let is_within_period = form_settings
            .answer_settings()
            .response_period()
            .is_within_period(Utc::now());

        if is_public_form && is_within_period {
            Ok(FormAnswer::new(user, form_id, title))
        } else {
            Err(Error::from(DomainError::Forbidden))
        }
    }
}
