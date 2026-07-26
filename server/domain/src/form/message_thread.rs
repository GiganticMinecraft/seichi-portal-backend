use chrono::{DateTime, Utc};
use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;

use crate::{
    account::models::{Role::Administrator, UserSnapshot},
    auth::Actor,
    form::{
        answer::{AnswerAuthor, AnswerEntry, AnswerId},
        message::{
            DeletedMessage, Message, MessageBody, MessageHistoryEntry, MessageId, MessagePost,
            can_read_deleted_message_history,
        },
    },
    types::authorization_guard::{
        Allowed, AuthorizationGuard, AuthorizationGuardDefinitions, AuthorizationRole, BelongsTo,
        Create, Delete, DeleteTransition, GuardedBy, ParentGuarded, Read, SelfGuarded, Update,
    },
};

#[derive(UnsafeFromRawParts, Clone, Debug, PartialEq)]
pub struct MessageThread {
    answer_id: AnswerId,
    answer_author: AnswerAuthor,
    messages: Vec<Message>,
}

struct MessageDeletionTarget {
    answer_id: AnswerId,
    message: Message,
}

impl AuthorizationRole for MessageDeletionTarget {
    type Role = ParentGuarded<MessageThread>;
}

impl BelongsTo<MessageThread> for MessageDeletionTarget {
    fn belongs_to(&self, parent: &MessageThread) -> bool {
        &self.answer_id == parent.answer_id() && self.message.belongs_to(parent)
    }
}

impl GuardedBy<MessageThread, Delete> for MessageDeletionTarget {
    fn is_allowed_for(&self, _parent: &MessageThread, actor: &Actor) -> bool {
        matches!(
            actor,
            Actor::AccountUser(user)
                if self.message.sender_id() == user.id() || user.role() == &Administrator
        )
    }
}

impl DeleteTransition for MessageDeletionTarget {
    type Created = DeletedMessage;
    type Context = DateTime<Utc>;

    fn transition(
        self,
        deleted_at: Self::Context,
        actor: &Actor,
    ) -> Result<Self::Created, DomainError> {
        let deleted_by = match actor {
            Actor::AccountUser(user) => UserSnapshot::from(user),
            _ => return Err(DomainError::Forbidden),
        };

        Ok(self.message.delete(self.answer_id, deleted_at, deleted_by))
    }
}

impl MessageThread {
    pub fn answer_id(&self) -> &AnswerId {
        &self.answer_id
    }

    pub(crate) fn answer_author(&self) -> &AnswerAuthor {
        &self.answer_author
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    #[cfg(test)]
    pub(crate) fn add_message(self, message: Message) -> Self {
        Self {
            messages: self.messages.into_iter().chain([message]).collect(),
            ..self
        }
    }

    pub fn find_message(&self, message_id: MessageId) -> Option<&Message> {
        self.messages.iter().find(|m| *m.id() == message_id)
    }
}

impl Allowed<AnswerEntry, Read> {
    pub fn message_thread(
        &self,
        messages: Vec<Message>,
    ) -> Result<Allowed<MessageThread, Read>, DomainError> {
        let thread = MessageThread {
            answer_id: *self.id(),
            answer_author: self.author().clone(),
            messages,
        };

        AuthorizationGuard::from(thread).try_read(self.actor().clone())
    }
}

impl Allowed<MessageThread, Update> {
    pub fn try_post_message(
        &self,
        message: Message,
    ) -> Result<Allowed<MessagePost, Create>, DomainError> {
        let answer_author_id = match self.answer_author() {
            AnswerAuthor::AuthenticatedUser(answer_author_id) => *answer_author_id,
            AnswerAuthor::Temporary(_) => {
                return Err(DomainError::MessagePostingNotSupportedForTemporaryAnswer);
            }
        };

        self.authorize_create(MessagePost::new(
            *self.answer_id(),
            answer_author_id,
            message,
        ))
    }

    pub fn authorize_message_update(
        &self,
        message_id: MessageId,
        new_body: MessageBody,
    ) -> Result<Allowed<Message, Update>, DomainError> {
        let message = self
            .value()
            .find_message(message_id)
            .ok_or(DomainError::NotFound)?
            .clone()
            .update_body(new_body);
        self.authorize_update(message)
    }

    pub fn authorize_message_delete(
        &self,
        message_id: MessageId,
        deleted_at: DateTime<Utc>,
    ) -> Result<Allowed<DeletedMessage, Create>, DomainError> {
        let message = self
            .value()
            .find_message(message_id)
            .ok_or(DomainError::NotFound)?
            .clone();
        let target = MessageDeletionTarget {
            answer_id: *self.answer_id(),
            message,
        };

        self.authorize_delete(target)?.delete(deleted_at)
    }
}

impl Allowed<MessageThread, Read> {
    pub fn can_read_deleted_message_history(&self) -> bool {
        can_read_deleted_message_history(self.actor())
    }

    pub fn authorize_message_history_entry(
        &self,
        history_entry: MessageHistoryEntry,
    ) -> Result<Allowed<MessageHistoryEntry, Read>, DomainError> {
        self.authorize_read(history_entry)
    }
}

fn is_answer_author_or_administrator(actor: &Actor, answer_author: &AnswerAuthor) -> bool {
    matches!(
        (actor, answer_author),
        (Actor::AccountUser(user), _)
            if user.role() == &Administrator
    ) || matches!(
        (actor, answer_author),
        (Actor::AccountUser(user), AnswerAuthor::AuthenticatedUser(answer_author_id))
            if user.id() == answer_author_id
    )
}

impl AuthorizationRole for MessageThread {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for MessageThread {
    fn can_create(&self, _actor: &Actor) -> bool {
        false
    }

    fn can_read(&self, actor: &Actor) -> bool {
        is_answer_author_or_administrator(actor, &self.answer_author)
    }

    fn can_update(&self, actor: &Actor) -> bool {
        is_answer_author_or_administrator(actor, &self.answer_author)
    }

    fn can_delete(&self, _actor: &Actor) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        account::models::{AccountUser, Role, UserId},
        form::answer::TemporaryAnswerAuthor,
        types::authorization_guard::{AuthorizationGuard, Delete, Read},
    };
    use uuid::Uuid;

    fn user_id(seed: &str) -> UserId {
        Uuid::parse_str(seed).unwrap().into()
    }

    fn answer_id(seed: &str) -> AnswerId {
        Uuid::parse_str(seed).unwrap().into()
    }

    fn active_user(name: &str, id: UserId, role: Role) -> AccountUser {
        AccountUser::new(name.to_string(), id, role)
    }

    fn thread_for_answer_author(answer_author_id: UserId) -> MessageThread {
        unsafe {
            MessageThread::from_raw_parts(
                answer_id("00000000-0000-7000-8000-000000000001"),
                AnswerAuthor::AuthenticatedUser(answer_author_id),
                Vec::new(),
            )
        }
    }

    fn message_body(value: &str) -> MessageBody {
        MessageBody::new(value.to_string().try_into().unwrap())
    }

    fn message_from(sender_id: UserId, body: &str) -> Message {
        Message::new(sender_id, message_body(body))
    }

    #[test]
    fn administrator_and_answer_author_can_read_and_update_message_thread() {
        let answer_author_id = user_id("00000000-0000-7000-8000-000000000201");
        let admin = Actor::from(active_user(
            "admin",
            user_id("00000000-0000-7000-8000-000000000202"),
            Role::Administrator,
        ));
        let answer_author = Actor::from(active_user(
            "answer_author",
            answer_author_id,
            Role::StandardUser,
        ));
        let other_user = Actor::from(active_user(
            "other_user",
            user_id("00000000-0000-7000-8000-000000000203"),
            Role::StandardUser,
        ));

        for actor in [admin, answer_author] {
            assert!(
                AuthorizationGuard::<_, Read>::from(thread_for_answer_author(answer_author_id))
                    .try_read(actor.clone())
                    .is_ok()
            );
            assert!(
                AuthorizationGuard::<_, Update>::from(thread_for_answer_author(answer_author_id))
                    .try_update(actor)
                    .is_ok()
            );
        }

        assert!(
            AuthorizationGuard::<_, Read>::from(thread_for_answer_author(answer_author_id))
                .try_read(other_user.clone())
                .is_err()
        );
        assert!(
            AuthorizationGuard::<_, Update>::from(thread_for_answer_author(answer_author_id))
                .try_update(other_user)
                .is_err()
        );
    }

    #[test]
    fn message_thread_cannot_be_deleted() {
        let answer_author_id = user_id("00000000-0000-7000-8000-000000000301");
        let admin = Actor::from(active_user(
            "admin",
            user_id("00000000-0000-7000-8000-000000000302"),
            Role::Administrator,
        ));
        let answer_author = Actor::from(active_user(
            "answer_author",
            answer_author_id,
            Role::StandardUser,
        ));

        assert!(
            AuthorizationGuard::<_, Delete>::from(thread_for_answer_author(answer_author_id))
                .try_delete(admin)
                .is_err()
        );
        assert!(
            AuthorizationGuard::<_, Delete>::from(thread_for_answer_author(answer_author_id))
                .try_delete(answer_author)
                .is_err()
        );
    }

    #[test]
    fn message_body_update_requires_message_sender_even_when_thread_is_updatable() {
        let answer_author_id = user_id("00000000-0000-7000-8000-000000000401");
        let admin_id = user_id("00000000-0000-7000-8000-000000000402");
        let admin = Actor::from(active_user("admin", admin_id, Role::Administrator));
        let answer_author = Actor::from(active_user(
            "answer_author",
            answer_author_id,
            Role::StandardUser,
        ));

        let answer_author_message = message_from(answer_author_id, "from answer author");
        let answer_author_message_id = *answer_author_message.id();
        let admin_message = message_from(admin_id, "from admin");
        let admin_message_id = *admin_message.id();
        let thread = thread_for_answer_author(answer_author_id)
            .add_message(answer_author_message)
            .add_message(admin_message);

        let updated_by_answer_author = AuthorizationGuard::<_, Update>::from(thread.clone())
            .try_update(answer_author.clone())
            .unwrap()
            .authorize_message_update(answer_author_message_id, message_body("updated"))
            .unwrap();

        assert_eq!(updated_by_answer_author.value().body().as_str(), "updated");

        let admin_updates_answer_author_message =
            AuthorizationGuard::<_, Update>::from(thread.clone())
                .try_update(admin)
                .unwrap()
                .authorize_message_update(
                    answer_author_message_id,
                    message_body("updated by admin"),
                );

        assert!(matches!(
            admin_updates_answer_author_message,
            Err(DomainError::Forbidden)
        ));

        let answer_author_updates_admin_message = AuthorizationGuard::<_, Update>::from(thread)
            .try_update(answer_author)
            .unwrap()
            .authorize_message_update(admin_message_id, message_body("updated by answer author"));

        assert!(matches!(
            answer_author_updates_admin_message,
            Err(DomainError::Forbidden)
        ));
    }

    #[test]
    fn message_body_update_returns_not_found_for_unknown_message_id() {
        let answer_author_id = user_id("00000000-0000-7000-8000-000000000501");
        let answer_author = Actor::from(active_user(
            "answer_author",
            answer_author_id,
            Role::StandardUser,
        ));
        let thread = thread_for_answer_author(answer_author_id);

        let result = AuthorizationGuard::<_, Update>::from(thread)
            .try_update(answer_author)
            .unwrap()
            .authorize_message_update(MessageId::new(), message_body("updated"));

        assert!(matches!(result, Err(DomainError::NotFound)));
    }

    #[test]
    fn message_post_requires_sender_to_match_the_thread_actor() {
        let answer_author_id = user_id("00000000-0000-7000-8000-000000000551");
        let another_user_id = user_id("00000000-0000-7000-8000-000000000552");
        let answer_author = Actor::from(active_user(
            "answer_author",
            answer_author_id,
            Role::StandardUser,
        ));
        let allowed_thread =
            AuthorizationGuard::<_, Update>::from(thread_for_answer_author(answer_author_id))
                .try_update(answer_author)
                .unwrap();

        let result = allowed_thread.try_post_message(message_from(
            another_user_id,
            "message with a different sender",
        ));

        assert!(matches!(result, Err(DomainError::Forbidden)));
    }

    #[test]
    fn authenticated_answer_thread_creates_post_with_answer_identity() {
        let answer_author_id = user_id("00000000-0000-7000-8000-000000000553");
        let actor = Actor::from(active_user(
            "answer_author",
            answer_author_id,
            Role::StandardUser,
        ));
        let thread = thread_for_answer_author(answer_author_id);
        let answer_id = *thread.answer_id();
        let message = message_from(answer_author_id, "message");
        let message_id = *message.id();

        let post = AuthorizationGuard::<_, Update>::from(thread)
            .try_update(actor)
            .unwrap()
            .try_post_message(message)
            .unwrap();

        assert_eq!(post.answer_id(), &answer_id);
        assert_eq!(post.answer_author_id(), &answer_author_id);
        assert_eq!(post.message().id(), &message_id);
        assert!(
            matches!(post.actor(), Actor::AccountUser(user) if post.message().sender_id() == user.id())
        );
    }

    #[test]
    fn temporary_answer_thread_is_admin_only_and_rejects_message_posts() {
        let admin_id = user_id("00000000-0000-7000-8000-000000000561");
        let admin = Actor::from(active_user("admin", admin_id, Role::Administrator));
        let temporary_author = AnswerAuthor::Temporary(TemporaryAnswerAuthor::new(
            "temporary_user".to_string(),
            "temporary@example.com".to_string(),
        ));
        let thread = unsafe {
            MessageThread::from_raw_parts(
                answer_id("00000000-0000-7000-8000-000000000562"),
                temporary_author,
                Vec::new(),
            )
        };
        let standard_user = Actor::from(active_user(
            "standard_user",
            user_id("00000000-0000-7000-8000-000000000563"),
            Role::StandardUser,
        ));

        assert!(
            AuthorizationGuard::<_, Read>::from(thread.clone())
                .try_read(standard_user)
                .is_err()
        );

        let thread = AuthorizationGuard::<_, Read>::from(thread)
            .try_read(admin)
            .unwrap()
            .try_into_update()
            .unwrap();
        let result = thread.try_post_message(message_from(admin_id, "forbidden"));

        assert!(matches!(
            result,
            Err(DomainError::MessagePostingNotSupportedForTemporaryAnswer)
        ));
    }

    #[test]
    fn message_delete_is_allowed_for_sender_or_administrator() {
        let answer_author_id = user_id("00000000-0000-7000-8000-000000000601");
        let admin_id = user_id("00000000-0000-7000-8000-000000000602");
        let admin = Actor::from(active_user("admin", admin_id, Role::Administrator));
        let answer_author = Actor::from(active_user(
            "answer_author",
            answer_author_id,
            Role::StandardUser,
        ));

        let answer_author_message = message_from(answer_author_id, "from answer author");
        let expected_answer_author_message = answer_author_message.clone();
        let answer_author_message_id = *answer_author_message.id();
        let admin_message = message_from(admin_id, "from admin");
        let admin_message_id = *admin_message.id();
        let thread = thread_for_answer_author(answer_author_id)
            .add_message(answer_author_message)
            .add_message(admin_message);
        let expected_answer_id = *thread.answer_id();
        let deleted_at = DateTime::from_timestamp(1_700_000_000, 0).unwrap();

        let sender_delete = AuthorizationGuard::<_, Update>::from(thread.clone())
            .try_update(answer_author.clone())
            .unwrap()
            .authorize_message_delete(answer_author_message_id, deleted_at);

        let sender_deleted = sender_delete.unwrap();
        assert_eq!(
            sender_deleted.value().message(),
            &expected_answer_author_message
        );
        assert_eq!(sender_deleted.value().answer_id(), &expected_answer_id);
        assert_eq!(sender_deleted.value().deleted_at(), &deleted_at);
        assert_eq!(
            sender_deleted.value().deleted_by(),
            &UserSnapshot::from(match sender_deleted.actor() {
                Actor::AccountUser(user) => user,
                _ => unreachable!(),
            })
        );

        let admin_delete = AuthorizationGuard::<_, Update>::from(thread.clone())
            .try_update(admin)
            .unwrap()
            .authorize_message_delete(answer_author_message_id, deleted_at);

        let admin_deleted = admin_delete.unwrap();
        assert_eq!(
            admin_deleted.value().message(),
            &expected_answer_author_message
        );
        assert_eq!(admin_deleted.value().answer_id(), &expected_answer_id);
        assert_eq!(admin_deleted.value().deleted_at(), &deleted_at);
        assert_eq!(
            admin_deleted.value().deleted_by(),
            &UserSnapshot::from(match admin_deleted.actor() {
                Actor::AccountUser(user) => user,
                _ => unreachable!(),
            })
        );

        let answer_author_deletes_admin_message = AuthorizationGuard::<_, Update>::from(thread)
            .try_update(answer_author)
            .unwrap()
            .authorize_message_delete(admin_message_id, deleted_at);

        assert!(matches!(
            answer_author_deletes_admin_message,
            Err(DomainError::Forbidden)
        ));
    }

    #[test]
    fn message_delete_returns_not_found_for_unknown_message_id() {
        let answer_author_id = user_id("00000000-0000-7000-8000-000000000701");
        let answer_author = Actor::from(active_user(
            "answer_author",
            answer_author_id,
            Role::StandardUser,
        ));
        let thread = thread_for_answer_author(answer_author_id);

        let result = AuthorizationGuard::<_, Update>::from(thread)
            .try_update(answer_author)
            .unwrap()
            .authorize_message_delete(MessageId::new(), Utc::now());

        assert!(matches!(result, Err(DomainError::NotFound)));
    }

    #[test]
    fn authorizing_message_delete_keeps_other_messages_in_the_thread() {
        let answer_author_id = user_id("00000000-0000-7000-8000-000000000801");
        let answer_author = Actor::from(active_user(
            "answer_author",
            answer_author_id,
            Role::StandardUser,
        ));
        let first_message = message_from(answer_author_id, "first");
        let first_message_id = *first_message.id();
        let second_message = message_from(answer_author_id, "second");
        let second_message_id = *second_message.id();
        let thread = thread_for_answer_author(answer_author_id)
            .add_message(first_message)
            .add_message(second_message);

        let allowed_thread = AuthorizationGuard::<_, Update>::from(thread)
            .try_update(answer_author)
            .unwrap();
        let _deletion_target = allowed_thread
            .authorize_message_delete(first_message_id, Utc::now())
            .unwrap();

        assert!(allowed_thread.find_message(first_message_id).is_some());
        assert!(allowed_thread.find_message(second_message_id).is_some());
    }

    #[test]
    fn message_history_entry_for_another_thread_is_rejected() {
        use crate::form::message::{MessageHistoryAction, MessageHistoryId};

        let answer_author_id = user_id("00000000-0000-7000-8000-000000000901");
        let answer_author = active_user("answer_author", answer_author_id, Role::StandardUser);
        let readable_thread =
            AuthorizationGuard::<_, Read>::from(thread_for_answer_author(answer_author_id))
                .try_read(Actor::from(answer_author.clone()))
                .unwrap();
        let snapshot = UserSnapshot::new(
            *answer_author.id(),
            answer_author.name().to_owned(),
            answer_author.role().to_owned(),
        );
        let history_entry = unsafe {
            MessageHistoryEntry::from_raw_parts(
                MessageHistoryId::new(),
                AnswerId::new(),
                MessageId::new(),
                snapshot.clone(),
                chrono::Utc::now(),
                MessageHistoryAction::Delete,
                message_body("state"),
                snapshot,
                chrono::Utc::now(),
            )
        };

        let result = readable_thread.authorize_message_history_entry(history_entry);

        assert!(matches!(result, Err(DomainError::NotFound)));
    }
}
