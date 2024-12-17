use derive_getters::Getters;
use serde::Deserialize;

use crate::{
    form::message::models::MessageId, types::authorization_guard::AuthorizationGuardDefinitions,
    user::models::User,
};

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
    ///     form::message::models::MessageId,
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

    /// [`Notification`] の各フィールドを指定して新しく作成します。
    ///
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::message::models::MessageId,
    ///     notification::models::{Notification, NotificationId, NotificationSource},
    ///     user::models::User,
    /// };
    /// use uuid::Uuid;
    ///
    /// let id = NotificationId::new();
    ///
    /// let source = NotificationSource::Message(MessageId::new());
    /// let recipient = User {
    ///     id: Uuid::new_v4(),
    ///     name: "Alice".to_string(),
    ///     role: Default::default(),
    /// };
    ///
    /// let notification = unsafe { Notification::from_raw_parts(id, source, recipient, false) };
    /// ```
    ///
    /// # Safety
    /// この関数は [`Notification`] のバリデーションをスキップするため、
    /// データベースからすでにバリデーションされているデータを読み出すときなど、
    /// データの信頼性が保証されている場合にのみ使用してください。
    pub unsafe fn from_raw_parts(
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

impl AuthorizationGuardDefinitions<Notification> for Notification {
    fn can_create(&self, actor: &User) -> bool {
        self.recipient().id == actor.id
    }

    fn can_read(&self, actor: &User) -> bool {
        self.recipient().id == actor.id
    }

    fn can_update(&self, actor: &User) -> bool {
        self.recipient().id == actor.id
    }

    fn can_delete(&self, actor: &User) -> bool {
        self.recipient().id == actor.id
    }
}
