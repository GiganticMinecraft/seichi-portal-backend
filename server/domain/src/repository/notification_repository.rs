use async_trait::async_trait;
use errors::Error;

use crate::notification::models::Notification;

#[async_trait]
pub trait NotificationRepository: Send + Sync + 'static {
    async fn create(&self, notification: &Notification) -> Result<(), Error>;
}
