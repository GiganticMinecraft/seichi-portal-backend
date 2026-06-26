use serde::{Deserialize, Serialize};

use crate::user::models::{TemporaryUser, UserId};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AnswerAuthor {
    AuthenticatedUser(UserId),
    TemporaryUser(TemporaryUser),
}

impl AnswerAuthor {
    pub fn authenticated_user_id(&self) -> Option<UserId> {
        match self {
            Self::AuthenticatedUser(user_id) => Some(*user_id),
            Self::TemporaryUser(_) => None,
        }
    }

    pub fn temporary_user(&self) -> Option<&TemporaryUser> {
        match self {
            Self::AuthenticatedUser(_) => None,
            Self::TemporaryUser(user) => Some(user),
        }
    }
}
