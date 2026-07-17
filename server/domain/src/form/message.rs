use chrono::{DateTime, Utc};
use derive_getters::Getters;
use deriving_via::DerivingVia;
use domain_derive::UnsafeFromRawParts;
use serde::{Deserialize, Serialize};
use types::non_empty_string::NonEmptyString;

use crate::{
    account::models::{Role, UserId, UserSnapshot},
    auth::Actor,
    form::{answer::AnswerId, is_administrator, message_thread::MessageThread},
    types::authorization_guard::{
        AuthorizationRole, BelongsTo, Create, Delete, GuardedBy, ParentGuarded, Read, Update,
    },
};

pub type MessageId = types::Id<Message>;
pub type MessageHistoryId = types::Id<MessageHistoryEntry>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageHistoryPagePosition(MessageHistoryId);

impl MessageHistoryPagePosition {
    pub fn new(id: MessageHistoryId) -> Self {
        Self(id)
    }

    pub fn id(&self) -> MessageHistoryId {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum MessageHistoryAction {
    Create,
    Update,
    Delete,
}

#[derive(UnsafeFromRawParts, Clone, Debug, PartialEq, Getters)]
pub struct MessageHistoryEntry {
    id: MessageHistoryId,
    #[getter(skip)]
    answer_id: AnswerId,
    message_id: MessageId,
    original_author: UserSnapshot,
    original_timestamp: DateTime<Utc>,
    action: MessageHistoryAction,
    body: MessageBody,
    operated_by: UserSnapshot,
    operated_at: DateTime<Utc>,
}

impl AuthorizationRole for MessageHistoryEntry {
    type Role = ParentGuarded<MessageThread>;
}

impl BelongsTo<MessageThread> for MessageHistoryEntry {
    fn belongs_to(&self, parent: &MessageThread) -> bool {
        &self.answer_id == parent.answer_id()
    }
}

impl GuardedBy<MessageThread, Read> for MessageHistoryEntry {
    fn is_allowed_for(&self, _parent: &MessageThread, actor: &Actor) -> bool {
        match self.action {
            MessageHistoryAction::Create | MessageHistoryAction::Update => true,
            MessageHistoryAction::Delete => can_read_deleted_message_history(actor),
        }
    }
}

pub(crate) fn can_read_deleted_message_history(actor: &Actor) -> bool {
    is_administrator(actor)
}

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

/// 既存のメッセージスレッドへ投稿するメッセージと、その所属先を表す。
#[derive(Getters, Debug)]
pub struct MessagePost {
    answer_id: crate::form::answer::AnswerId,
    message: Message,
}

impl MessagePost {
    pub(crate) fn new(answer_id: crate::form::answer::AnswerId, message: Message) -> Self {
        Self { answer_id, message }
    }

    pub fn into_message(self) -> Message {
        self.message
    }
}

impl AuthorizationRole for MessagePost {
    type Role = ParentGuarded<MessageThread>;
}

impl BelongsTo<MessageThread> for MessagePost {
    fn belongs_to(&self, parent: &MessageThread) -> bool {
        self.answer_id() == parent.answer_id()
    }
}

impl GuardedBy<MessageThread, Create> for MessagePost {
    fn is_allowed_for(&self, _parent: &MessageThread, actor: &Actor) -> bool {
        matches!(actor, Actor::AccountUser(user) if user.id() == self.message.sender_id())
    }
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

    pub(crate) fn delete(
        self,
        answer_id: AnswerId,
        deleted_at: DateTime<Utc>,
        deleted_by: UserSnapshot,
    ) -> DeletedMessage {
        DeletedMessage {
            answer_id,
            message: self,
            deleted_at,
            deleted_by,
        }
    }
}

/// 削除されたメッセージと、削除時点の操作情報を表す。
#[derive(Getters, Debug, PartialEq)]
pub struct DeletedMessage {
    answer_id: AnswerId,
    message: Message,
    deleted_at: DateTime<Utc>,
    deleted_by: UserSnapshot,
}

impl AuthorizationRole for DeletedMessage {
    type Role = ParentGuarded<MessageThread>;
}

impl BelongsTo<MessageThread> for DeletedMessage {
    fn belongs_to(&self, parent: &MessageThread) -> bool {
        &self.answer_id == parent.answer_id() && self.message.belongs_to(parent)
    }
}

impl GuardedBy<MessageThread, Delete> for DeletedMessage {
    fn is_allowed_for(&self, _parent: &MessageThread, actor: &Actor) -> bool {
        matches!(
            actor,
            Actor::AccountUser(user)
                if (user.id() == self.message.sender_id()
                    || user.role() == &Role::Administrator)
                    && user.id() == self.deleted_by.id()
                    && user.name() == self.deleted_by.name()
                    && user.role() == self.deleted_by.role()
        )
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
        matches!(actor, Actor::AccountUser(user) if self.sender_id() == user.id())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        account::models::{AccountUser, Role},
        form::answer::AnswerId,
        types::authorization_guard::{AuthorizationGuard, Read, Update},
    };
    use errors::domain::DomainError;
    use uuid::Uuid;

    #[test]
    fn message_update_requires_message_to_belong_to_thread() {
        let user_id: UserId = Uuid::new_v4().into();
        let actor = Actor::from(AccountUser::new(
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

    #[test]
    fn message_delete_rejects_deleted_by_snapshot_that_differs_from_actor() {
        let user = AccountUser::new(
            "sender".to_string(),
            UserId::from(Uuid::new_v4()),
            Role::StandardUser,
        );
        let answer_id = AnswerId::new();
        let message = Message::new(
            *user.id(),
            MessageBody::new("delete me".to_string().try_into().unwrap()),
        );
        let thread = MessageThread::new(answer_id, *user.id()).add_message(message.clone());
        let actor = Actor::from(user.clone());
        let deleted_at = DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let snapshot_with_different_name = UserSnapshot::new(
            *user.id(),
            "different-name".to_string(),
            user.role().to_owned(),
        );
        let snapshot_with_different_role =
            UserSnapshot::new(*user.id(), user.name().to_owned(), Role::Administrator);

        let different_name =
            message
                .clone()
                .delete(answer_id, deleted_at, snapshot_with_different_name);
        let different_role = message.delete(answer_id, deleted_at, snapshot_with_different_role);

        assert!(!different_name.is_allowed_for(&thread, &actor));
        assert!(!different_role.is_allowed_for(&thread, &actor));
    }

    #[test]
    fn message_history_read_authorization_depends_on_action_and_actor_role() {
        let answer_id = AnswerId::new();
        let author = AccountUser::new(
            "author".to_string(),
            UserId::from(Uuid::new_v4()),
            Role::StandardUser,
        );
        let administrator = AccountUser::new(
            "administrator".to_string(),
            UserId::from(Uuid::new_v4()),
            Role::Administrator,
        );
        let thread = MessageThread::new(answer_id, *author.id());
        let standard_readable_thread = AuthorizationGuard::<_, Read>::from(thread.clone())
            .try_read(Actor::from(author.clone()))
            .unwrap();
        let admin_readable_thread = AuthorizationGuard::<_, Read>::from(thread)
            .try_read(Actor::from(administrator))
            .unwrap();
        let snapshot = UserSnapshot::new(
            *author.id(),
            author.name().to_owned(),
            author.role().to_owned(),
        );
        let history_entry = |action| unsafe {
            MessageHistoryEntry::from_raw_parts(
                MessageHistoryId::new(),
                answer_id,
                MessageId::new(),
                snapshot.clone(),
                Utc::now(),
                action,
                MessageBody::new("state".to_string().try_into().unwrap()),
                snapshot.clone(),
                Utc::now(),
            )
        };

        let standard_create = standard_readable_thread
            .authorize_message_history_entry(history_entry(MessageHistoryAction::Create));
        let standard_update = standard_readable_thread
            .authorize_message_history_entry(history_entry(MessageHistoryAction::Update));
        let standard_delete = standard_readable_thread
            .authorize_message_history_entry(history_entry(MessageHistoryAction::Delete));
        let admin_update = admin_readable_thread
            .authorize_message_history_entry(history_entry(MessageHistoryAction::Update));
        let admin_delete = admin_readable_thread
            .authorize_message_history_entry(history_entry(MessageHistoryAction::Delete));

        assert!(!standard_readable_thread.can_read_deleted_message_history());
        assert!(admin_readable_thread.can_read_deleted_message_history());
        assert!(standard_create.is_ok());
        assert!(standard_update.is_ok());
        assert!(matches!(standard_delete, Err(DomainError::Forbidden)));
        assert!(admin_update.is_ok());
        assert!(admin_delete.is_ok());
    }
}
