use derive_getters::Getters;
use serde::Deserialize;

use crate::{form::models::MessageId, user::models::User};

#[derive(Deserialize, Debug)]
pub enum NotificationSource {
    Message(MessageId),
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

    pub fn from_raw_parts(
        id: NotificationId,
        source: NotificationSource,
        recipient: User,
        is_read: bool,
    ) -> Self {
        Self {
            id,
            source,
            recipient,
            is_read,
        }
    }
}
