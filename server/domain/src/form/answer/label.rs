use derive_getters::Getters;
use domain_derive::UnsafeFromRawParts;
#[cfg(test)]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};
use types::non_empty_string::NonEmptyString;

use crate::{
    types::authorization_guard::{AuthorizationGuardDefinitions, AuthorizationRole, SelfGuarded},
    user::models::{Actor, Role, User},
};

pub type AnswerLabelId = types::Id<AnswerLabel>;

#[cfg_attr(test, derive(Arbitrary))]
#[derive(UnsafeFromRawParts, Serialize, Deserialize, Getters, Debug, PartialEq)]
pub struct AnswerLabel {
    id: AnswerLabelId,
    name: NonEmptyString,
}

impl AnswerLabel {
    pub fn new(name: NonEmptyString) -> Self {
        Self {
            id: AnswerLabelId::new(),
            name,
        }
    }

    pub fn renamed(self, name: NonEmptyString) -> Self {
        Self { name, ..self }
    }
}

impl AuthorizationRole for AnswerLabel {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for AnswerLabel {
    fn can_create(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(actor)) if actor.role() == &Role::Administrator)
    }

    fn can_read(&self, _actor: &Actor) -> bool {
        true
    }

    fn can_update(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(actor)) if actor.role() == &Role::Administrator)
    }

    fn can_delete(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(actor)) if actor.role() == &Role::Administrator)
    }
}
