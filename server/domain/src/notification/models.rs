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
    /// [`Notification`] を新しく作成します。
    ///
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::models::MessageId,
    ///     notification::models::{Notification, NotificationSource},
    ///     user::models::User,
    /// };
    ///
    /// let source = NotificationSource::Message(MessageId::new());
    /// let recipient = User {
    ///     id: Default::default(),
    ///     name: "Alice".to_string(),
    ///     role: Default::default(),
    /// };
    /// let notification = Notification::new(source, recipient);
    ///
    /// assert!(!notification.is_read());
    /// ```
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
