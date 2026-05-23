use derive_getters::Getters;

use crate::{
    types::authorization_guard::AuthorizationGuardDefinitions,
    user::models::{Role, User, UserId},
};

#[derive(Getters, Debug)]
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

    pub fn from_raw_parts(recipient_id: UserId, is_send_message_notification: bool) -> Self {
        Self {
            recipient_id,
            is_send_message_notification,
        }
    }
}

impl AuthorizationGuardDefinitions for NotificationPreference {
    fn can_create(&self, actor: &User) -> bool {
        self.recipient_id() == &actor.id || actor.role == Role::Administrator
    }

    fn can_read(&self, actor: &User) -> bool {
        self.recipient_id() == &actor.id || actor.role == Role::Administrator
    }

    fn can_update(&self, actor: &User) -> bool {
        self.recipient_id() == &actor.id || actor.role == Role::Administrator
    }

    fn can_delete(&self, _actor: &User) -> bool {
        // NOTE: 明示的に通知設定を削除することはない(削除されるのは User が削除されたときのみ)
        false
    }
}
