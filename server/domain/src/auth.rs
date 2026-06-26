use crate::{account::models::AccountUser, form::answer::TemporaryAnswerAuthor};

#[derive(Debug, Clone, PartialEq)]
pub enum Actor {
    AccountUser(AccountUser),
    TemporaryAnswerAuthor(TemporaryAnswerAuthor),
    Anonymous,
    System,
}

impl From<AccountUser> for Actor {
    fn from(user: AccountUser) -> Self {
        Self::AccountUser(user)
    }
}

impl From<TemporaryAnswerAuthor> for Actor {
    fn from(user: TemporaryAnswerAuthor) -> Self {
        Self::TemporaryAnswerAuthor(user)
    }
}
