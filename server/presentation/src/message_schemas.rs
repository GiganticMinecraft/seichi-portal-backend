use domain::form::models::AnswerId;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct PostedMessageSchema {
    pub related_answer_id: AnswerId,
    pub body: String,
}
