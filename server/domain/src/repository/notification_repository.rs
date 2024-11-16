use async_trait::async_trait;
use errors::Error;
use uuid::Uuid;

use crate::notification::models::Notification;

#[async_trait]
pub trait NotificationRepository: Send + Sync + 'static {
    async fn create(&self, notification: &Notification) -> Result<(), Error>;
    async fn fetch_by_recipient_id(&self, recipient_id: Uuid) -> Result<Vec<Notification>, Error>;
}
