use chrono::{DateTime, Utc};
use derive_getters::Getters;
use domain_derive::UnsafeFromRawParts;
use strum_macros::{Display, EnumString};

use crate::{
    account::models::{Role, UserId},
    auth::Actor,
    form::answer::AnswerId,
    types::authorization_guard::{AuthorizationGuardDefinitions, AuthorizationRole, SelfGuarded},
};

pub type NotificationId = types::Id<Notification>;

#[derive(Clone, Copy, Debug, Display, EnumString, Eq, PartialEq)]
pub enum NotificationType {
    #[strum(serialize = "MESSAGE_RECEIVED")]
    MessageReceived,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NotificationContent {
    title: String,
    body: String,
    url: String,
}

impl NotificationContent {
    pub fn new(title: impl Into<String>, body: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            url: url.into(),
        }
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn body(&self) -> &str {
        &self.body
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn to_message(&self) -> String {
        [self.title.as_str(), self.body.as_str(), self.url.as_str()].join("\n")
    }
}

#[derive(UnsafeFromRawParts, Getters, Debug, Clone, PartialEq)]
pub struct Notification {
    id: NotificationId,
    recipient_id: UserId,
    notification_type: NotificationType,
    related_answer_id: AnswerId,
    content: NotificationContent,
    created_at: DateTime<Utc>,
    read_at: Option<DateTime<Utc>>,
}

impl Notification {
    pub fn new(
        recipient_id: UserId,
        notification_type: NotificationType,
        related_answer_id: AnswerId,
        content: NotificationContent,
        created_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id: NotificationId::new(),
            recipient_id,
            notification_type,
            related_answer_id,
            content,
            created_at,
            read_at: None,
        }
    }

    pub fn mark_as_read(self, read_at: DateTime<Utc>) -> Self {
        Self {
            read_at: self.read_at.or(Some(read_at)),
            ..self
        }
    }
}

impl AuthorizationRole for Notification {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for Notification {
    fn can_create(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::System)
    }

    fn can_read(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::AccountUser(actor) if self.recipient_id == *actor.id())
    }

    fn can_update(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::AccountUser(actor) if self.recipient_id == *actor.id())
    }

    fn can_delete(&self, _actor: &Actor) -> bool {
        false
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NotificationPagePosition(NotificationId);

impl NotificationPagePosition {
    pub fn new(id: NotificationId) -> Self {
        Self(id)
    }

    pub fn id(self) -> NotificationId {
        self.0
    }
}

#[derive(UnsafeFromRawParts, Getters, Debug, Clone)]
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
            Actor::AccountUser(actor)
                if self.recipient_id() == actor.id() || actor.role() == &Role::Administrator
        ) || matches!(actor, Actor::System)
    }

    fn can_read(&self, actor: &Actor) -> bool {
        matches!(
            actor,
            Actor::AccountUser(actor)
                if self.recipient_id() == actor.id() || actor.role() == &Role::Administrator
        ) || matches!(actor, Actor::System)
    }

    fn can_update(&self, actor: &Actor) -> bool {
        matches!(
            actor,
            Actor::AccountUser(actor)
                if self.recipient_id() == actor.id() || actor.role() == &Role::Administrator
        )
    }

    fn can_delete(&self, _actor: &Actor) -> bool {
        // NOTE: 明示的に通知設定を削除することはない(削除されるのは User が削除されたときのみ)
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        account::models::AccountUser,
        types::authorization_guard::{AuthorizationGuard, Read, Update},
    };
    use uuid::Uuid;

    fn user(name: &str, role: Role) -> AccountUser {
        AccountUser::new(name.to_string(), UserId::from(Uuid::new_v4()), role)
    }

    #[test]
    fn notification_is_readable_and_updatable_only_by_recipient() {
        let recipient = user("recipient", Role::StandardUser);
        let administrator = user("administrator", Role::Administrator);
        let notification = Notification::new(
            *recipient.id(),
            NotificationType::MessageReceived,
            AnswerId::new(),
            NotificationContent::new("title", "body", "https://example.com"),
            Utc::now(),
        );

        assert!(
            AuthorizationGuard::<_, Read>::from(notification.clone())
                .try_read(Actor::from(recipient.clone()))
                .is_ok()
        );
        assert!(
            AuthorizationGuard::<_, Update>::from(notification.clone())
                .try_update(Actor::from(recipient))
                .is_ok()
        );
        assert!(
            AuthorizationGuard::<_, Read>::from(notification.clone())
                .try_read(Actor::from(administrator.clone()))
                .is_err()
        );
        assert!(
            AuthorizationGuard::<_, Update>::from(notification)
                .try_update(Actor::from(administrator))
                .is_err()
        );
    }

    #[test]
    fn mark_as_read_preserves_the_first_read_timestamp() {
        let created_at = Utc::now();
        let first_read_at = created_at + chrono::Duration::minutes(1);
        let second_read_at = first_read_at + chrono::Duration::minutes(1);
        let notification = Notification::new(
            UserId::from(Uuid::new_v4()),
            NotificationType::MessageReceived,
            AnswerId::new(),
            NotificationContent::new("title", "body", "https://example.com"),
            created_at,
        );

        let notification = notification.mark_as_read(first_read_at);
        let notification = notification.mark_as_read(second_read_at);

        assert_eq!(*notification.read_at(), Some(first_read_at));
    }
}
