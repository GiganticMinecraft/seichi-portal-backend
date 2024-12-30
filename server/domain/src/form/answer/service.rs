use async_trait::async_trait;
use chrono::Utc;
use errors::{domain::DomainError, Error};

use crate::types::verified::{Verified, Verifier};
use crate::{
    form::{
        answer::models::{AnswerEntry, AnswerTitle},
        models::{FormId, Visibility},
    },
    repository::form::form_repository::FormRepository,
    user::models::User,
};

pub struct PostAnswerEntriesVerifier<'a, FormRepo: FormRepository> {
    pub form_repo: &'a FormRepo,
    pub actor: &'a User,
    pub answer_entry: AnswerEntry,
}

#[async_trait]
impl<FormRepo: FormRepository> Verifier<AnswerEntry> for PostAnswerEntriesVerifier<'_, FormRepo> {
    async fn verify(self) -> Result<Verified<AnswerEntry>, Error> {
        let target = self.answer_entry;

        let form = self
            .form_repo
            .get(*target.form_id())
            .await?
            .ok_or(DomainError::NotFound)?
            .try_into_read(self.actor)?;

        let form_settings = form.settings();

        let is_public_form = *form_settings.visibility() == Visibility::PUBLIC;
        let is_within_period = form_settings
            .answer_settings()
            .response_period()
            .is_within_period(Utc::now());

        if is_public_form && is_within_period {
            Ok(Self::new(target))
        } else {
            Err(Error::from(DomainError::Forbidden))
        }
    }
}
