use async_trait::async_trait;
use domain::{
    notification::models::Notification, repository::notification_repository::NotificationRepository,
};
use errors::Error;

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
}
