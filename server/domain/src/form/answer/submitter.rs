use chrono::{DateTime, Utc};
use errors::domain::DomainError;

use crate::account::models::{AccountUser, AnswerSubmissionRestriction};

#[derive(Clone, Debug, PartialEq)]
pub struct AnswerSubmitter {
    user: AccountUser,
}

impl AnswerSubmitter {
    pub fn try_new(
        user: AccountUser,
        restriction: Option<AnswerSubmissionRestriction>,
        now: DateTime<Utc>,
    ) -> Result<Self, DomainError> {
        if let Some(restriction) = restriction {
            if restriction.user_id() != user.id() {
                return Err(DomainError::InvalidEntity {
                    message: "answer submission restriction must belong to the submitter"
                        .to_string(),
                });
            }

            if !restriction.is_active_at(now) {
                return Ok(Self { user });
            }

            return Err(DomainError::AnswerSubmissionRestricted {
                reason: restriction.reason().to_owned().into_inner().into_inner(),
                expires_at: *restriction.expires_at(),
            });
        }

        Ok(Self { user })
    }

    pub fn user(&self) -> &AccountUser {
        &self.user
    }

    pub fn into_user(self) -> AccountUser {
        self.user
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account::models::{AnswerSubmissionRestrictionReason, Role, UserId};
    use uuid::Uuid;

    fn user_id(seed: u128) -> UserId {
        UserId::from(Uuid::from_u128(seed))
    }

    fn active_user(name: &str, id: UserId, role: Role) -> AccountUser {
        AccountUser::new(name.to_string(), id, role)
    }

    #[test]
    fn answer_submitter_is_created_when_user_has_no_active_restriction() {
        let user = active_user("user", user_id(1), Role::StandardUser);

        assert!(AnswerSubmitter::try_new(user, None, Utc::now()).is_ok());
    }

    #[test]
    fn answer_submitter_rejects_active_restriction() {
        let now = Utc::now();
        let user = active_user("user", user_id(1), Role::StandardUser);
        let restriction = AnswerSubmissionRestriction::new(
            *user.id(),
            AnswerSubmissionRestrictionReason::new("spam".to_string().try_into().unwrap()),
            user_id(2),
            now,
            None,
        )
        .unwrap();

        let result = AnswerSubmitter::try_new(user, Some(restriction), now);

        assert_eq!(
            result,
            Err(DomainError::AnswerSubmissionRestricted {
                reason: "spam".to_string(),
                expires_at: None,
            })
        );
    }

    #[test]
    fn answer_submitter_ignores_expired_restriction() {
        let now = Utc::now();
        let user = active_user("user", user_id(1), Role::StandardUser);
        let restriction = AnswerSubmissionRestriction::new(
            *user.id(),
            AnswerSubmissionRestrictionReason::new("spam".to_string().try_into().unwrap()),
            user_id(2),
            now - chrono::Duration::hours(2),
            Some(now - chrono::Duration::hours(1)),
        )
        .unwrap();

        assert!(AnswerSubmitter::try_new(user, Some(restriction), now).is_ok());
    }

    #[test]
    fn answer_submitter_rejects_restriction_for_different_user() {
        let now = Utc::now();
        let user = active_user("user", user_id(1), Role::StandardUser);
        let restriction = AnswerSubmissionRestriction::new(
            user_id(2),
            AnswerSubmissionRestrictionReason::new("spam".to_string().try_into().unwrap()),
            user_id(3),
            now,
            None,
        )
        .unwrap();

        let result = AnswerSubmitter::try_new(user, Some(restriction), now);

        assert_eq!(
            result,
            Err(DomainError::InvalidEntity {
                message: "answer submission restriction must belong to the submitter".to_string(),
            })
        );
    }
}
