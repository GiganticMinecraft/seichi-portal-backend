use domain::{
    notification::models::Notification, repository::notification_repository::NotificationRepository,
};
use errors::Error;
use uuid::Uuid;

pub struct NotificationUseCase<'a, NotificationRepo: NotificationRepository> {
    pub repository: &'a NotificationRepo,
}

impl<R: NotificationRepository> NotificationUseCase<'_, R> {
    pub async fn fetch_notifications(
        &self,
        recipient_id: Uuid,
    ) -> Result<Vec<Notification>, Error> {
        self.repository.fetch_by_recipient_id(recipient_id).await
    }
}
