use async_trait::async_trait;
use domain::{
    notification::models::Notification, repository::notification_repository::NotificationRepository,
};
use errors::Error;
use uuid::Uuid;

use crate::{
    database::components::{DatabaseComponents, NotificationDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> NotificationRepository for Repository<Client> {
    async fn create(&self, notification: &Notification) -> Result<(), Error> {
        self.client
            .notification()
            .create(notification)
            .await
            .map_err(Into::into)
    }

    async fn fetch_by_recipient_id(&self, recipient_id: Uuid) -> Result<Vec<Notification>, Error> {
        self.client
            .notification()
            .fetch_by_recipient(recipient_id)
            .await
            .map(|notifications| {
                notifications
                    .into_iter()
                    .map(TryInto::try_into)
                    .collect::<Result<Vec<_>, _>>()
            })?
            .map_err(Into::into)
    }
}
