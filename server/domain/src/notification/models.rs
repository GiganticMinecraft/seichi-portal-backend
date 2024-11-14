use derive_getters::Getters;
use serde::Deserialize;

use crate::{form::models::MessageId, user::models::User};

#[derive(Deserialize, Debug)]
pub enum NotificationSource {
    Message { message_id: MessageId },
}

pub type NotificationId = types::Id<Notification>;

#[derive(Deserialize, Getters, Debug)]
pub struct Notification {
    id: NotificationId,
    source: NotificationSource,
    recipient: User,
    is_read: bool,
}

impl Notification {
    pub fn new(source: NotificationSource, recipient: User) -> Self {
        Self {
            id: NotificationId::new(),
            source,
            recipient,
            is_read: false,
        }
    }
}
