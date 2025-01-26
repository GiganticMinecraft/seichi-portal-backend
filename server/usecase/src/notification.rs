use domain::{
    notification::models::{Notification, NotificationId},
    repository::notification_repository::NotificationRepository,
    types::{authorization_guard::AuthorizationGuard, authorization_guard_with_context::Read},
    user::models::User,
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
    ) -> Result<Vec<AuthorizationGuard<Notification, Read>>, Error> {
        self.repository.fetch_by_recipient_id(recipient_id).await
    }

    pub async fn update_notification_read_status(
        &self,
        actor: &User,
        notification_id_with_is_read: Vec<(NotificationId, bool)>,
    ) -> Result<Vec<AuthorizationGuard<Notification, Read>>, Error> {
        let (notification_id, is_read): (Vec<NotificationId>, Vec<bool>) =
            notification_id_with_is_read.into_iter().unzip();

        let notifications = self
            .repository
            .fetch_by_notification_ids(notification_id.to_owned())
            .await?;

        self.repository
            .update_read_status(
                actor,
                notifications
                    .into_iter()
                    .map(AuthorizationGuard::<_, Read>::into_update)
                    .zip(is_read.into_iter())
                    .collect::<Vec<_>>(),
            )
            .await?;

        self.repository
            .fetch_by_notification_ids(notification_id)
            .await
    }
}
