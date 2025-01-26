use chrono::Utc;

use crate::{
    form::{
        answer::{
            models::AnswerEntry,
            settings::models::{AnswerVisibility, ResponsePeriod},
        },
        models::Visibility,
    },
    types::authorization_guard_with_context::AuthorizationGuardWithContextDefinitions,
    user::models::{Role, User},
};

#[derive(Debug)]
pub struct AnswerEntryAuthorizationContext {
    pub form_visibility: Visibility,
    pub response_period: ResponsePeriod,
    pub answer_visibility: AnswerVisibility,
}

impl AuthorizationGuardWithContextDefinitions<AnswerEntry, AnswerEntryAuthorizationContext>
    for AnswerEntry
{
    fn can_create(&self, actor: &User, context: &AnswerEntryAuthorizationContext) -> bool {
        let is_public_form = context.form_visibility == Visibility::PUBLIC;
        let is_within_period = context.response_period.is_within_period(Utc::now());

        is_public_form && is_within_period || actor.role == Role::Administrator
    }

    fn can_read(&self, actor: &User, context: &AnswerEntryAuthorizationContext) -> bool {
        self.user().id == actor.id
            || context.answer_visibility == AnswerVisibility::PUBLIC
            || actor.role == Role::Administrator
    }

    fn can_update(&self, _actor: &User, _context: &AnswerEntryAuthorizationContext) -> bool {
        false
    }

    fn can_delete(&self, actor: &User, _context: &AnswerEntryAuthorizationContext) -> bool {
        actor.role == Role::Administrator
    }
}
