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

#[derive(Clone, Debug)]
pub enum AnswerEntryActor {
    AuthenticatedUser(User),
    TemporaryUser,
}

impl From<User> for AnswerEntryActor {
    fn from(user: User) -> Self {
        Self::AuthenticatedUser(user)
    }
}

impl AuthorizationGuardWithContextDefinitions<AnswerEntryAuthorizationContext, AnswerEntryActor>
    for AnswerEntry
{
    fn can_create(
        &self,
        actor: &AnswerEntryActor,
        context: &AnswerEntryAuthorizationContext,
    ) -> bool {
        let is_within_period = context.response_period.is_within_period(Utc::now());

        match (self.author(), actor) {
            (
                AnswerAuthor::AuthenticatedUser(user_id),
                AnswerEntryActor::AuthenticatedUser(user),
            ) => {
                let is_public_form = context.form_visibility == Visibility::PUBLIC;
                *user_id == user.id
                    && ((is_public_form && is_within_period) || user.role == Role::Administrator)
            }
            (AnswerAuthor::TemporaryUser(_), AnswerEntryActor::TemporaryUser) => {
                context.allow_temporary_answers && is_within_period
            }
            _ => false,
        }
    }

    fn can_read(
        &self,
        actor: &AnswerEntryActor,
        context: &AnswerEntryAuthorizationContext,
    ) -> bool {
        match actor {
            AnswerEntryActor::AuthenticatedUser(user) => {
                self.author().authenticated_user_id() == Some(user.id)
                    || context.answer_visibility == AnswerVisibility::PUBLIC
                    || user.role == Role::Administrator
            }
            AnswerEntryActor::TemporaryUser => false,
        }
    }

    fn can_update(
        &self,
        _actor: &AnswerEntryActor,
        _context: &AnswerEntryAuthorizationContext,
    ) -> bool {
        false
    }

    fn can_delete(
        &self,
        actor: &AnswerEntryActor,
        _context: &AnswerEntryAuthorizationContext,
    ) -> bool {
        matches!(
            actor,
            AnswerEntryActor::AuthenticatedUser(User {
                role: Role::Administrator,
                ..
            })
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

        assert!(answer.can_create(&super::AnswerEntryActor::TemporaryUser, &context));
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

        assert!(!answer.can_create(&super::AnswerEntryActor::TemporaryUser, &context));
    }
}
