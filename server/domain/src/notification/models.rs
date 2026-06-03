use derive_getters::Getters;

use crate::{
    types::authorization_guard::{AuthorizationGuardDefinitions, AuthorizationRole, SelfGuarded},
    user::models::{Actor, Role, User, UserId},
};

#[derive(Debug)]
pub enum NotificationType {
    MessageReceived,
}

#[derive(Debug)]
pub struct NotificationContent {
    lines: Vec<String>,
}

impl NotificationContent {
    pub fn new(lines: Vec<String>) -> Self {
        Self { lines }
    }

    pub fn to_message(&self) -> String {
        self.lines.join("\n")
    }
}

#[derive(Getters, Debug, Clone)]
pub struct NotificationPreference {
    recipient_id: UserId,
    is_send_message_notification: bool,
}

impl NotificationPreference {
    pub fn new(recipient_id: UserId) -> Self {
        Self {
            recipient_id,
            is_send_message_notification: false,
        }
    }

    pub fn update_send_message_notification(self, is_send_message_notification: bool) -> Self {
        Self {
            is_send_message_notification,
            ..self
        }
    }

    /// [`NotificationPreference`] を永続化済みのフィールド値から復元します。
    ///
    /// # Safety
    /// 新規作成ではなく、データベースなど信頼できる永続化済みデータの復元にのみ使用してください。
    pub unsafe fn from_raw_parts(recipient_id: UserId, is_send_message_notification: bool) -> Self {
        Self {
            recipient_id,
            is_send_message_notification,
        }
    }
}

impl NotificationPreference {
    pub fn is_enabled(&self, notification_type: &NotificationType) -> bool {
        match notification_type {
            NotificationType::MessageReceived => self.is_send_message_notification,
        }
    }
}

impl AuthorizationRole for NotificationPreference {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for NotificationPreference {
    fn can_create(&self, actor: &Actor) -> bool {
        matches!(
            actor,
            Actor::User(User::ActiveUser(actor))
                if self.recipient_id() == actor.id() || actor.role() == &Role::Administrator
        ) || matches!(actor, Actor::System)
    }

    fn can_read(&self, actor: &Actor) -> bool {
        matches!(
            actor,
            Actor::User(User::ActiveUser(actor))
                if self.recipient_id() == actor.id() || actor.role() == &Role::Administrator
        ) || matches!(actor, Actor::System)
    }

    fn can_update(&self, actor: &Actor) -> bool {
        matches!(
            actor,
            Actor::User(User::ActiveUser(actor))
                if self.recipient_id() == actor.id() || actor.role() == &Role::Administrator
        )
    }

    fn can_delete(&self, _actor: &Actor) -> bool {
        // NOTE: 明示的に通知設定を削除することはない(削除されるのは User が削除されたときのみ)
        false
    }
}
