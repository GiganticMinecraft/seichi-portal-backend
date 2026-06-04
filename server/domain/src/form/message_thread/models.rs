use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;

use crate::{
    form::{
        answer::models::AnswerId,
        message::models::{Message, MessageId},
    },
    types::authorization_guard::{
        Allowed, AuthorizationGuardDefinitions, AuthorizationRole, Authorizes, Delete, SelfGuarded,
        Update,
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

    fn apply_message_update(
        self,
        message_id: MessageId,
        new_body: String,
    ) -> Result<Self, DomainError> {
        let messages = self
            .messages
            .into_iter()
            .map(|m| {
                if *m.id() == message_id {
                    m.update_body(new_body.clone())
                } else {
                    Ok(m)
                }
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { messages, ..self })
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

impl Authorizes<Message, Update> for MessageThread {
    fn check(&self, actor: &Actor, message: &Message) -> Result<(), DomainError> {
        match actor {
            Actor::User(User::ActiveUser(user)) if message.sender_id() == user.id() => Ok(()),
            _ => Err(DomainError::Forbidden),
        }
    }
}

impl Authorizes<Message, Delete> for MessageThread {
    fn check(&self, actor: &Actor, message: &Message) -> Result<(), DomainError> {
        match actor {
            Actor::User(User::ActiveUser(user))
                if message.sender_id() == user.id() || user.role() == &Administrator =>
            {
                Ok(())
            }
            _ => Err(DomainError::Forbidden),
        }
    }
}

impl Allowed<MessageThread, Update> {
    pub fn update_message_body(
        self,
        message_id: MessageId,
        new_body: String,
    ) -> Result<Self, DomainError> {
        let message = self
            .value()
            .find_message(message_id)
            .ok_or(DomainError::NotFound)?
            .clone();
        self.authorize_update(message)?;
        self.try_map(|thread| thread.apply_message_update(message_id, new_body))
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
        is_answer_author_or_administrator(actor, &self.answer_author_id)
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
