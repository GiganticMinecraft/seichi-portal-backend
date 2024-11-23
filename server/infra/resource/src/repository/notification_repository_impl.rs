use async_trait::async_trait;
use domain::{
    notification::models::{Notification, NotificationId},
    repository::notification_repository::NotificationRepository,
    types::authorization_guard::{AuthorizationGuard, Create, Read, Update},
    user::models::User,
};
use errors::Error;
use itertools::Itertools;
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

    async fn fetch_by_recipient_id(
        &self,
        recipient_id: Uuid,
    ) -> Result<Vec<AuthorizationGuard<Notification, Read>>, Error> {
        self.client
            .notification()
            .fetch_by_recipient(recipient_id)
            .await
            .map(|notifications| {
                notifications
                    .into_iter()
                    .flat_map(TryInto::<Notification>::try_into)
                    .map(Into::<AuthorizationGuard<Notification, Create>>::into)
                    .map(AuthorizationGuard::<_, Create>::into_read)
                    .collect_vec()
            })
            .map_err(Into::into)
    }

    async fn fetch_by_notification_ids(
        &self,
        notification_ids: Vec<NotificationId>,
    ) -> Result<Vec<AuthorizationGuard<Notification, Read>>, Error> {
        self.client
            .notification()
            .fetch_by_notification_ids(notification_ids)
            .await?
            .into_iter()
            .map(TryInto::<Notification>::try_into)
            .map(|notification| {
                notification
                    .map(Into::into)
                    .map(AuthorizationGuard::<_, Create>::into_read)
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    async fn update_read_status(
        &self,
        actor: &User,
        notifications: Vec<(AuthorizationGuard<Notification, Update>, bool)>,
    ) -> Result<(), Error> {
        let update_targets = notifications
            .into_iter()
            .map(|(notification, is_read)| {
                notification.try_update(actor, |notification| {
                    (notification.id().to_owned(), is_read)
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::<Error>::into)?;

        self.client
            .notification()
            .update_read_status(update_targets)
            .await
            .map_err(Into::into)
    }
}
