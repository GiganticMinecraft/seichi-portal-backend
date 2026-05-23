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

impl AnswerEntryAuthorizationContext {
    pub fn can_create_temporary(&self) -> bool {
        self.response_period.is_within_period(Utc::now())
    }
}

impl AuthorizationGuardWithContextDefinitions<AnswerEntryAuthorizationContext> for AnswerEntry {
    fn can_create(&self, actor: &User, context: &AnswerEntryAuthorizationContext) -> bool {
        let is_public_form = context.form_visibility == Visibility::PUBLIC;
        let is_within_period = context.response_period.is_within_period(Utc::now());

        (is_public_form && is_within_period) || actor.role == Role::Administrator
    }

    fn can_read(&self, actor: &User, context: &AnswerEntryAuthorizationContext) -> bool {
        self.author().authenticated_user_id() == Some(actor.id)
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

#[cfg(test)]
mod tests {
    use chrono::{Duration, Utc};

    use crate::form::{
        answer::{
            service::AnswerEntryAuthorizationContext,
            settings::models::{AnswerVisibility, ResponsePeriod},
        },
        models::Visibility,
    };

    #[test]
    fn temporary_answer_creation_does_not_depend_on_form_visibility() {
        let context = AnswerEntryAuthorizationContext {
            form_visibility: Visibility::PRIVATE,
            response_period: ResponsePeriod::try_new(None, None).unwrap(),
            answer_visibility: AnswerVisibility::PRIVATE,
        };

        assert!(context.can_create_temporary());
    }

    #[test]
    fn temporary_answer_creation_respects_response_period() {
        let context = AnswerEntryAuthorizationContext {
            form_visibility: Visibility::PUBLIC,
            response_period: ResponsePeriod::try_new(
                Some(Utc::now() - Duration::days(2)),
                Some(Utc::now() - Duration::days(1)),
            )
            .unwrap(),
            answer_visibility: AnswerVisibility::PRIVATE,
        };

        assert!(!context.can_create_temporary());
    }
}
