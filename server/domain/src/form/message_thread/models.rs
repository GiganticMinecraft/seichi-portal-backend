use errors::domain::DomainError;

use crate::{
    form::{
        answer::models::AnswerId,
        message::models::{Message, MessageId},
    },
    types::authorization_guard::AuthorizationGuardDefinitions,
    user::models::{Actor, Role::Administrator, User, UserId},
};

pub type MessageThreadId = types::Id<MessageThread>;

#[derive(Clone, Debug, PartialEq)]
pub struct MessageThread {
    id: MessageThreadId,
    answer_id: AnswerId,
    answer_author_id: UserId,
    messages: Vec<Message>,
}

impl MessageThread {
    pub fn new(answer_id: AnswerId, answer_author_id: UserId) -> Self {
        Self {
            id: MessageThreadId::new(),
            answer_id,
            answer_author_id,
            messages: Vec::new(),
        }
    }

    pub fn from_raw_parts(
        id: MessageThreadId,
        answer_id: AnswerId,
        answer_author_id: UserId,
        messages: Vec<Message>,
    ) -> Self {
        Self {
            id,
            answer_id,
            answer_author_id,
            messages,
        }
    }

    pub fn id(&self) -> &MessageThreadId {
        &self.id
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

    pub fn add_message(mut self, message: Message) -> Self {
        self.messages.push(message);
        self
    }

    pub fn find_message(&self, message_id: MessageId) -> Option<&Message> {
        self.messages.iter().find(|m| *m.id() == message_id)
    }

    pub fn update_message_body(
        self,
        message_id: MessageId,
        actor: &Actor,
        new_body: String,
    ) -> Result<Self, DomainError> {
        let message = self.find_message(message_id).ok_or(DomainError::NotFound)?;
        match actor {
            Actor::User(User::ActiveUser(user)) if message.sender_id() == user.id() => {}
            _ => return Err(DomainError::Forbidden),
        }
        let messages = self
            .messages
            .into_iter()
            .map(|m| {
                if *m.id() == message_id {
                    unsafe {
                        Message::from_raw_parts(
                            *m.id(),
                            *m.sender_id(),
                            new_body.clone(),
                            *m.timestamp(),
                        )
                    }
                } else {
                    m
                }
            })
            .collect();
        Ok(Self { messages, ..self })
    }

    pub fn remove_message(self, message_id: MessageId, actor: &Actor) -> Result<Self, DomainError> {
        let message = self.find_message(message_id).ok_or(DomainError::NotFound)?;
        match actor {
            Actor::User(User::ActiveUser(user)) if message.sender_id() == user.id() => {}
            _ => return Err(DomainError::Forbidden),
        }
        let messages = self
            .messages
            .into_iter()
            .filter(|m| *m.id() != message_id)
            .collect();
        Ok(Self { messages, ..self })
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
