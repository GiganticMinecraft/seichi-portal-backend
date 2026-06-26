use chrono::{DateTime, Utc};
use errors::domain::DomainError;

use crate::user::models::{ActiveUser, AnswerSubmissionRestriction};

#[derive(Clone, Debug, PartialEq)]
pub struct AnswerSubmitter {
    user: ActiveUser,
}

impl AnswerSubmitter {
    pub fn try_new(
        user: ActiveUser,
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

    pub fn user(&self) -> &ActiveUser {
        &self.user
    }

    pub fn into_user(self) -> ActiveUser {
        self.user
    }
}
