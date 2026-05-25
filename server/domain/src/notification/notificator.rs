use async_trait::async_trait;
use errors::Error;

use crate::{
    notification::models::{NotificationContent, NotificationPreference, NotificationType},
    user::models::UserId,
};

#[async_trait]
pub trait Notificator: Send + Sync {
    async fn notify(
        &self,
        recipient: UserId,
        notification_type: NotificationType,
        notification_preference: &NotificationPreference,
        content: &NotificationContent,
    ) -> Result<(), Error>;
}
