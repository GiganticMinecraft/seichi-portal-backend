use chrono::{DateTime, Utc};

use crate::{form::models::AnswerId, user::models::User};

pub type MessageId = types::Id<Message>;

#[derive(Debug)]
pub struct Message {
    pub id: MessageId,
    pub related_answer_id: AnswerId,
    pub contents: Vec<MessageContent>,
}

#[derive(Debug)]
pub struct MessageContent {
    pub posted_user: User,
    pub body: String,
    pub timestamp: DateTime<Utc>,
}
