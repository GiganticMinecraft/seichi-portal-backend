use async_trait::async_trait;
use chrono::Utc;
use derive_getters::Getters;
use errors::{domain::DomainError, Error};

use crate::form::answer::settings::models::AnswerVisibility;
use crate::form::models::FormSettings;
use crate::types::authorization_guard_with_context::AuthorizationGuardWithContextDefinitions;
use crate::user::models::Role;
use crate::{
    form::{answer::models::AnswerEntry, models::Visibility},
    repository::form::form_repository::FormRepository,
    types::verified::{Verified, Verifier},
    user::models::User,
};

#[derive(Getters, Debug)]
pub struct AnswerEntryAuthorizationContext<'a> {
    form_settings: &'a FormSettings,
}

impl<'a> AnswerEntryAuthorizationContext<'a> {
    pub fn new(form_settings: &'a FormSettings) -> Self {
        Self { form_settings }
    }
}

impl AuthorizationGuardWithContextDefinitions<AnswerEntry, AnswerEntryAuthorizationContext<'_>>
    for AnswerEntry
{
    fn can_create(&self, actor: &User, context: &AnswerEntryAuthorizationContext) -> bool {
        let is_public_form = *context.form_settings.visibility() == Visibility::PUBLIC;
        let is_within_period = context
            .form_settings
            .answer_settings()
            .response_period()
            .is_within_period(Utc::now());

        is_public_form && is_within_period || actor.role == Role::Administrator
    }

    fn can_read(&self, actor: &User, context: &AnswerEntryAuthorizationContext) -> bool {
        self.user().id == actor.id
            || *context.form_settings.answer_settings().visibility() == AnswerVisibility::PUBLIC
            || actor.role == Role::Administrator
    }

    fn can_update(&self, _actor: &User, _context: &AnswerEntryAuthorizationContext) -> bool {
        false
    }

    fn can_delete(&self, actor: &User, _context: &AnswerEntryAuthorizationContext) -> bool {
        actor.role == Role::Administrator
    }
}

pub struct PostAnswerEntriesVerifier<'a, FormRepo: FormRepository> {
    pub form_repo: &'a FormRepo,
    pub actor: &'a User,
}

#[async_trait]
impl<FormRepo: FormRepository> Verifier<AnswerEntry> for PostAnswerEntriesVerifier<'_, FormRepo> {
    async fn verify(self, target: AnswerEntry) -> Result<Verified<AnswerEntry>, Error> {
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
            Ok(Self::new_verified(target))
        } else {
            Err(Error::from(DomainError::Forbidden))
        }
    }
}
