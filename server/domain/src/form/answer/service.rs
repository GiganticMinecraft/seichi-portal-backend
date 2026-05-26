use chrono::Utc;

use crate::{
    form::{
        answer::{
            models::{AnswerAuthor, AnswerEntry},
            settings::models::{AnswerVisibility, ResponsePeriod},
        },
        models::{FormSettings, Visibility},
    },
    types::authorization_guard_with_context::AuthorizationGuardWithContextDefinitions,
    user::models::{Actor, Role, User},
};

#[derive(Debug)]
pub struct AnswerEntryAuthorizationContext {
    pub form_visibility: Visibility,
    pub response_period: ResponsePeriod,
    pub answer_visibility: AnswerVisibility,
    pub allow_temporary_answers: bool,
}

impl AnswerEntryAuthorizationContext {
    pub fn from_form_settings(settings: &FormSettings) -> Self {
        Self {
            form_visibility: *settings.visibility(),
            response_period: settings.answer_settings().response_period().to_owned(),
            answer_visibility: *settings.answer_settings().visibility(),
            allow_temporary_answers: settings.allow_temporary_answers(),
        }
    }
}

impl AuthorizationGuardWithContextDefinitions<AnswerEntryAuthorizationContext> for AnswerEntry {
    fn can_create(&self, actor: &Actor, context: &AnswerEntryAuthorizationContext) -> bool {
        let is_within_period = context.response_period.is_within_period(Utc::now());

        match (self.author(), actor) {
            (AnswerAuthor::AuthenticatedUser(user_id), Actor::User(User::ActiveUser(user))) => {
                let is_public_form = context.form_visibility == Visibility::PUBLIC;
                *user_id == *user.id()
                    && ((is_public_form && is_within_period) || user.role() == &Role::Administrator)
            }
            (AnswerAuthor::TemporaryUser(_), Actor::User(User::TemporaryUser(_))) => {
                context.form_visibility == Visibility::PUBLIC
                    && context.allow_temporary_answers
                    && is_within_period
            }
            _ => false,
        }
    }

    fn can_read(&self, actor: &Actor, context: &AnswerEntryAuthorizationContext) -> bool {
        match actor {
            Actor::User(User::ActiveUser(user)) => {
                self.author().authenticated_user_id() == Some(*user.id())
                    || context.answer_visibility == AnswerVisibility::PUBLIC
                    || user.role() == &Role::Administrator
            }
            _ => false,
        }
    }

    fn can_update(&self, _actor: &Actor, _context: &AnswerEntryAuthorizationContext) -> bool {
        false
    }

    fn can_delete(&self, actor: &Actor, _context: &AnswerEntryAuthorizationContext) -> bool {
        matches!(
            actor,
            Actor::User(User::ActiveUser(user)) if user.role() == &Role::Administrator
        )
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
    use crate::types::authorization_guard_with_context::AuthorizationGuardWithContextDefinitions;
    use crate::user::models::Actor;

    #[test]
    fn temporary_answer_creation_requires_public_form() {
        let context = AnswerEntryAuthorizationContext {
            form_visibility: Visibility::PRIVATE,
            response_period: ResponsePeriod::try_new(None, None).unwrap(),
            answer_visibility: AnswerVisibility::PRIVATE,
            allow_temporary_answers: true,
        };
        let answer = crate::form::answer::models::AnswerEntry::new(
            crate::form::answer::models::AnswerAuthor::TemporaryUser(
                crate::user::models::TemporaryUser::new("guest".to_string(), "contact".to_string()),
            ),
            crate::form::models::FormId::new(),
            crate::form::answer::models::AnswerTitle::default(),
            crate::form::answer::models::PostedAnswerContents::try_new(&[], vec![]).unwrap(),
        );

        assert!(!answer.can_create(
            &Actor::from(crate::user::models::TemporaryUser::new(
                "guest".to_string(),
                "contact".to_string()
            )),
            &context
        ));
    }

    #[test]
    fn temporary_answer_creation_succeeds_on_public_form_within_period() {
        let context = AnswerEntryAuthorizationContext {
            form_visibility: Visibility::PUBLIC,
            response_period: ResponsePeriod::try_new(None, None).unwrap(),
            answer_visibility: AnswerVisibility::PRIVATE,
            allow_temporary_answers: true,
        };
        let answer = crate::form::answer::models::AnswerEntry::new(
            crate::form::answer::models::AnswerAuthor::TemporaryUser(
                crate::user::models::TemporaryUser::new("guest".to_string(), "contact".to_string()),
            ),
            crate::form::models::FormId::new(),
            crate::form::answer::models::AnswerTitle::default(),
            crate::form::answer::models::PostedAnswerContents::try_new(&[], vec![]).unwrap(),
        );

        assert!(answer.can_create(
            &Actor::from(crate::user::models::TemporaryUser::new(
                "guest".to_string(),
                "contact".to_string()
            )),
            &context
        ));
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
            allow_temporary_answers: true,
        };
        let answer = crate::form::answer::models::AnswerEntry::new(
            crate::form::answer::models::AnswerAuthor::TemporaryUser(
                crate::user::models::TemporaryUser::new("guest".to_string(), "contact".to_string()),
            ),
            crate::form::models::FormId::new(),
            crate::form::answer::models::AnswerTitle::default(),
            crate::form::answer::models::PostedAnswerContents::try_new(&[], vec![]).unwrap(),
        );

        assert!(!answer.can_create(
            &Actor::from(crate::user::models::TemporaryUser::new(
                "guest".to_string(),
                "contact".to_string()
            )),
            &context
        ));
    }
}
