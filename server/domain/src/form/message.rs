use chrono::{DateTime, Utc};
use derive_getters::Getters;
use deriving_via::DerivingVia;
use domain_derive::UnsafeFromRawParts;
use serde::{Deserialize, Serialize};
use types::non_empty_string::NonEmptyString;

use crate::{
    form::message_thread::MessageThread,
    types::authorization_guard::{
        AuthorizationRole, BelongsTo, Delete, GuardedBy, ParentGuarded, Update,
    },
    user::models::{Actor, Role::Administrator, User, UserId},
};

pub type MessageId = types::Id<Message>;

#[derive(DerivingVia, Debug, PartialEq)]
#[deriving(Clone, From, Into, IntoInner, Serialize, Deserialize)]
pub struct MessageBody(NonEmptyString);

impl MessageBody {
    pub fn new(body: NonEmptyString) -> Self {
        Self(body)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl AuthorizationRole for Message {
    type Role = ParentGuarded<MessageThread>;
}

#[derive(UnsafeFromRawParts, Getters, Clone, PartialEq, Debug, Serialize, Deserialize)]
pub struct Message {
    id: MessageId,
    sender_id: UserId,
    body: MessageBody,
    timestamp: DateTime<Utc>,
}

impl Message {
    pub fn new(sender_id: UserId, body: MessageBody) -> Self {
        Self {
            id: MessageId::new(),
            sender_id,
            body,
            timestamp: Utc::now(),
        }
    }

    pub fn update_body(self, body: MessageBody) -> Self {
        Self { body, ..self }
    }
}

impl BelongsTo<MessageThread> for Message {
    fn belongs_to(&self, parent: &MessageThread) -> bool {
        parent
            .messages()
            .iter()
            .any(|message| message.id() == self.id())
    }
}

impl GuardedBy<MessageThread, Update> for Message {
    fn is_allowed_for(&self, _parent: &MessageThread, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(user)) if self.sender_id() == user.id())
    }
}

impl GuardedBy<MessageThread, Delete> for Message {
    fn is_allowed_for(&self, _parent: &MessageThread, actor: &Actor) -> bool {
        matches!(
            actor,
            Actor::User(User::ActiveUser(user))
                if self.sender_id() == user.id() || user.role() == &Administrator
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        form::answer::AnswerId,
        types::authorization_guard::{AuthorizationGuard, Update},
        user::models::{ActiveUser, Role},
    };
    use uuid::Uuid;

    #[test]
    fn message_update_requires_message_to_belong_to_thread() {
        let user_id: UserId = Uuid::new_v4().into();
        let actor = Actor::from(ActiveUser::new(
            "sender".to_string(),
            user_id,
            Role::StandardUser,
        ));
        let answer_id: AnswerId = Uuid::new_v4().into();
        let thread = MessageThread::new(answer_id, user_id).add_message(Message::new(
            user_id,
            MessageBody::new("owned message".to_string().try_into().unwrap()),
        ));
        let foreign_message = Message::new(
            user_id,
            MessageBody::new("foreign message".to_string().try_into().unwrap()),
        );
        let allowed_thread = AuthorizationGuard::<_, Update>::from(thread)
            .try_update(actor)
            .unwrap();

        let result = allowed_thread.authorize_update(foreign_message);

        assert!(result.is_err());
    }
}
