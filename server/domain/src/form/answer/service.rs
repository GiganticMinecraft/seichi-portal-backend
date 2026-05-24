use chrono::Utc;

use crate::{
    form::{
        answer::{
            models::{AnswerAuthor, AnswerEntry},
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
    pub allow_temporary_answers: bool,
}

impl AuthorizationGuardWithContextDefinitions<AnswerEntryAuthorizationContext> for AnswerEntry {
    fn can_create(&self, actor: &User, context: &AnswerEntryAuthorizationContext) -> bool {
        let is_within_period = context.response_period.is_within_period(Utc::now());

        match (self.author(), actor) {
            (AnswerAuthor::AuthenticatedUser(user_id), User::ActiveUser(user)) => {
                let is_public_form = context.form_visibility == Visibility::PUBLIC;
                *user_id == *user.id()
                    && ((is_public_form && is_within_period) || user.role() == &Role::Administrator)
            }
            (AnswerAuthor::TemporaryUser(_), User::TemporaryUser(_)) => {
                context.allow_temporary_answers && is_within_period
            }
            _ => false,
        }
    }

    fn can_read(&self, actor: &User, context: &AnswerEntryAuthorizationContext) -> bool {
        match actor {
            User::ActiveUser(user) => {
                self.author().authenticated_user_id() == Some(*user.id())
                    || context.answer_visibility == AnswerVisibility::PUBLIC
                    || user.role() == &Role::Administrator
            }
            User::TemporaryUser(_) => false,
        }
    }

    fn can_update(&self, _actor: &User, _context: &AnswerEntryAuthorizationContext) -> bool {
        false
    }

    fn can_delete(&self, actor: &User, _context: &AnswerEntryAuthorizationContext) -> bool {
        matches!(
            actor,
            User::ActiveUser(user) if user.role() == &Role::Administrator
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

    #[test]
    fn temporary_answer_creation_does_not_depend_on_form_visibility() {
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

        assert!(answer.can_create(
            &crate::user::models::User::TemporaryUser(crate::user::models::TemporaryUser::new(
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
            &crate::user::models::User::TemporaryUser(crate::user::models::TemporaryUser::new(
                "guest".to_string(),
                "contact".to_string()
            )),
            &context
        ));
    }
}
