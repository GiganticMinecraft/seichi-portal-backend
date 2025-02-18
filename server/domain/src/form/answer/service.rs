use chrono::Utc;

use crate::{
    form::{
        answer::{
            models::{AnswerEntry, FormAnswerContent},
            settings::models::{AnswerVisibility, ResponsePeriod},
        },
        models::Visibility,
    },
    types::authorization_guard_with_context::{
        Actions, AuthorizationGuardWithContext, AuthorizationGuardWithContextDefinitions,
    },
    user::models::{Role, User},
};

#[derive(Debug)]
pub struct AnswerEntryAuthorizationContext {
    pub form_visibility: Visibility,
    pub response_period: ResponsePeriod,
    pub answer_visibility: AnswerVisibility,
}

// NOTE: FormAnswerEntry は FormAnswerContent と同じ条件でアクセス制御を行う
impl AuthorizationGuardWithContextDefinitions<AnswerEntryAuthorizationContext> for AnswerEntry {
    fn can_create(&self, actor: &User, context: &AnswerEntryAuthorizationContext) -> bool {
        let is_public_form = context.form_visibility == Visibility::PUBLIC;
        let is_within_period = context.response_period.is_within_period(Utc::now());

        (is_public_form && is_within_period) || actor.role == Role::Administrator
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

pub struct FormAnswerContentAuthorizationContext<'a, Action: Actions> {
    pub answer_entry_authorization_context: &'a AnswerEntryAuthorizationContext,
    pub answer_entry:
        &'a AuthorizationGuardWithContext<AnswerEntry, Action, AnswerEntryAuthorizationContext>,
}

// NOTE: FormAnswerContent は FormAnswerEntry と同じ条件でアクセス制御を行う
impl<Action: Actions>
    AuthorizationGuardWithContextDefinitions<FormAnswerContentAuthorizationContext<'_, Action>>
    for FormAnswerContent
{
    fn can_create(
        &self,
        actor: &User,
        context: &FormAnswerContentAuthorizationContext<'_, Action>,
    ) -> bool {
        context
            .answer_entry
            .can_create(actor, context.answer_entry_authorization_context)
    }

    fn can_read(
        &self,
        actor: &User,
        context: &FormAnswerContentAuthorizationContext<'_, Action>,
    ) -> bool {
        context
            .answer_entry
            .can_read(actor, context.answer_entry_authorization_context)
    }

    fn can_update(
        &self,
        actor: &User,
        context: &FormAnswerContentAuthorizationContext<'_, Action>,
    ) -> bool {
        context
            .answer_entry
            .can_update(actor, context.answer_entry_authorization_context)
    }

    fn can_delete(
        &self,
        actor: &User,
        context: &FormAnswerContentAuthorizationContext<'_, Action>,
    ) -> bool {
        context
            .answer_entry
            .can_delete(actor, context.answer_entry_authorization_context)
    }
}
