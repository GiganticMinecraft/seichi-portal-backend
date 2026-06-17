use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;

use crate::{
    form::{
        answer::AnswerId,
        message::{Message, MessageBody, MessageId},
    },
    types::authorization_guard::{
        Allowed, AuthorizationGuardDefinitions, AuthorizationRole, Delete, SelfGuarded, Update,
    },
    user::models::{Actor, Role::Administrator, User, UserId},
};

#[derive(UnsafeFromRawParts, Clone, Debug, PartialEq)]
pub struct MessageThread {
    answer_id: AnswerId,
    answer_author_id: UserId,
    messages: Vec<Message>,
}

impl MessageThread {
    pub fn new(answer_id: AnswerId, answer_author_id: UserId) -> Self {
        Self {
            answer_id,
            answer_author_id,
            messages: Vec::new(),
        }
    }

    pub fn answer_id(&self) -> &AnswerId {
        &self.answer_id
    }

    pub fn answer_author_id(&self) -> &UserId {
        &self.answer_author_id
    }

    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    pub fn add_message(self, message: Message) -> Self {
        Self {
            messages: self.messages.into_iter().chain([message]).collect(),
            ..self
        }
    }

    pub fn find_message(&self, message_id: MessageId) -> Option<&Message> {
        self.messages.iter().find(|m| *m.id() == message_id)
    }

    fn apply_message_update(self, message_id: MessageId, new_body: MessageBody) -> Self {
        let messages = self
            .messages
            .into_iter()
            .map(|m| {
                if *m.id() == message_id {
                    m.update_body(new_body.clone())
                } else {
                    m
                }
            })
            .collect();
        Self { messages, ..self }
    }

    fn apply_message_removal(self, message_id: MessageId) -> Self {
        Self {
            messages: self
                .messages
                .into_iter()
                .filter(|m| *m.id() != message_id)
                .collect(),
            ..self
        }
    }
}

impl Allowed<MessageThread, Update> {
    pub fn update_message_body(
        self,
        message_id: MessageId,
        new_body: MessageBody,
    ) -> Result<Self, DomainError> {
        let message = self
            .value()
            .find_message(message_id)
            .ok_or(DomainError::NotFound)?
            .clone();
        self.authorize_update(message)?;
        Ok(self.map(|thread| thread.apply_message_update(message_id, new_body)))
    }

    pub fn authorize_message_delete(
        &self,
        message_id: MessageId,
    ) -> Result<Allowed<Message, Delete>, DomainError> {
        let message = self
            .value()
            .find_message(message_id)
            .ok_or(DomainError::NotFound)?
            .clone();
        self.authorize_delete(message)
    }

    pub fn remove_message(self, message: Allowed<Message, Delete>) -> Self {
        self.map(|thread| thread.apply_message_removal(*message.value().id()))
    }
}

fn is_answer_author_or_administrator(actor: &Actor, answer_author_id: &UserId) -> bool {
    matches!(
        actor,
        Actor::User(User::ActiveUser(user))
            if user.role() == &Administrator
                || *user.id() == *answer_author_id
    )
}

impl AuthorizationRole for MessageThread {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for MessageThread {
    fn can_create(&self, actor: &Actor) -> bool {
        matches!(
            actor,
            Actor::User(User::ActiveUser(user)) if user.role() == &Administrator
        )
    }

    fn can_read(&self, actor: &Actor) -> bool {
        is_answer_author_or_administrator(actor, &self.answer_author_id)
    }

    fn can_update(&self, actor: &Actor) -> bool {
        is_answer_author_or_administrator(actor, &self.answer_author_id)
    }

    fn can_delete(&self, _actor: &Actor) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        types::authorization_guard::{AuthorizationGuard, Create, Delete, Read},
        user::models::{ActiveUser, Role, TemporaryUser},
    };
    use uuid::Uuid;

    fn user_id(seed: &str) -> UserId {
        Uuid::parse_str(seed).unwrap().into()
    }

    fn answer_id(seed: &str) -> AnswerId {
        Uuid::parse_str(seed).unwrap().into()
    }

    fn active_user(name: &str, id: UserId, role: Role) -> ActiveUser {
        ActiveUser::new(name.to_string(), id, role)
    }

    fn thread_for_answer_author(answer_author_id: UserId) -> MessageThread {
        MessageThread::new(
            answer_id("00000000-0000-7000-8000-000000000001"),
            answer_author_id,
        )
    }

    fn message_body(value: &str) -> MessageBody {
        MessageBody::new(value.to_string().try_into().unwrap())
    }

    fn message_from(sender_id: UserId, body: &str) -> Message {
        Message::new(sender_id, message_body(body))
    }

    #[test]
    fn only_administrator_can_create_message_thread() {
        let answer_author_id = user_id("00000000-0000-7000-8000-000000000101");
        let admin = Actor::from(active_user(
            "admin",
            user_id("00000000-0000-7000-8000-000000000102"),
            Role::Administrator,
        ));
        let answer_author = Actor::from(active_user(
            "answer_author",
            answer_author_id,
            Role::StandardUser,
        ));
        let other_user = Actor::from(active_user(
            "other_user",
            user_id("00000000-0000-7000-8000-000000000103"),
            Role::StandardUser,
        ));
        let temporary_user = Actor::from(TemporaryUser::new(
            "temporary_user".to_string(),
            "temporary@example.com".to_string(),
        ));

        assert!(
            AuthorizationGuard::<_, Create>::from(thread_for_answer_author(answer_author_id))
                .try_create(admin)
                .is_ok()
        );
        assert!(
            AuthorizationGuard::<_, Create>::from(thread_for_answer_author(answer_author_id))
                .try_create(answer_author)
                .is_err()
        );
        assert!(
            AuthorizationGuard::<_, Create>::from(thread_for_answer_author(answer_author_id))
                .try_create(other_user)
                .is_err()
        );
        assert!(
            AuthorizationGuard::<_, Create>::from(thread_for_answer_author(answer_author_id))
                .try_create(temporary_user)
                .is_err()
        );
        assert!(
            AuthorizationGuard::<_, Create>::from(thread_for_answer_author(answer_author_id))
                .try_create(Actor::User(User::Anonymous))
                .is_err()
        );
        assert!(
            AuthorizationGuard::<_, Create>::from(thread_for_answer_author(answer_author_id))
                .try_create(Actor::System)
                .is_err()
        );
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
            .update_message_body(answer_author_message_id, message_body("updated"))
            .unwrap();

        assert_eq!(
            updated_by_answer_author
                .find_message(answer_author_message_id)
                .unwrap()
                .body()
                .as_str(),
            "updated"
        );

        let admin_updates_answer_author_message =
            AuthorizationGuard::<_, Update>::from(thread.clone())
                .try_update(admin)
                .unwrap()
                .update_message_body(answer_author_message_id, message_body("updated by admin"));

        assert!(matches!(
            admin_updates_answer_author_message,
            Err(DomainError::Forbidden)
        ));

        let answer_author_updates_admin_message = AuthorizationGuard::<_, Update>::from(thread)
            .try_update(answer_author)
            .unwrap()
            .update_message_body(admin_message_id, message_body("updated by answer author"));

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
            .update_message_body(MessageId::new(), message_body("updated"));

        assert!(matches!(result, Err(DomainError::NotFound)));
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
        let answer_author_message_id = *answer_author_message.id();
        let admin_message = message_from(admin_id, "from admin");
        let admin_message_id = *admin_message.id();
        let thread = thread_for_answer_author(answer_author_id)
            .add_message(answer_author_message)
            .add_message(admin_message);

        let sender_delete = AuthorizationGuard::<_, Update>::from(thread.clone())
            .try_update(answer_author.clone())
            .unwrap()
            .authorize_message_delete(answer_author_message_id);

        assert!(sender_delete.is_ok());

        let admin_delete = AuthorizationGuard::<_, Update>::from(thread.clone())
            .try_update(admin)
            .unwrap()
            .authorize_message_delete(answer_author_message_id);

        assert!(admin_delete.is_ok());

        let answer_author_deletes_admin_message = AuthorizationGuard::<_, Update>::from(thread)
            .try_update(answer_author)
            .unwrap()
            .authorize_message_delete(admin_message_id);

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
            .authorize_message_delete(MessageId::new());

        assert!(matches!(result, Err(DomainError::NotFound)));
    }

    #[test]
    fn remove_message_removes_only_authorized_message() {
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
        let deletion_target = allowed_thread
            .authorize_message_delete(first_message_id)
            .unwrap();
        let updated_thread = allowed_thread.remove_message(deletion_target);

        assert!(updated_thread.find_message(first_message_id).is_none());
        assert!(updated_thread.find_message(second_message_id).is_some());
    }
}
