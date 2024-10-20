use chrono::{DateTime, Utc};
use domain::form::models::AnswerId;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Debug)]
pub struct PostedMessageSchema {
    pub related_answer_id: AnswerId,
    pub body: String,
}

#[derive(Serialize, Debug)]
pub struct GetMessageResponseSchema {
    pub messages: Vec<MessageContentSchema>,
}

#[derive(Serialize, Debug)]
pub struct MessageContentSchema {
    pub body: String,
    pub sender: SenderSchema,
    pub timestamp: DateTime<Utc>,
}

#[derive(Serialize, Debug)]
pub struct SenderSchema {
    pub uuid: String,
    pub name: String,
    pub role: String,
}
