use async_trait::async_trait;
use domain::{
    notification::models::{Notification, NotificationId, NotificationSettings},
    repository::notification_repository::NotificationRepository,
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard_with_context::{Create, Read, Update},
    },
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
        Ok(self
            .client
            .notification()
            .fetch_by_recipient(recipient_id)
            .await?
            .into_iter()
            .flat_map(TryInto::<Notification>::try_into)
            .map(Into::<AuthorizationGuard<Notification, Create>>::into)
            .map(AuthorizationGuard::<_, Create>::into_read)
            .collect_vec())
    }

    async fn fetch_by_notification_ids(
        &self,
        notification_ids: Vec<NotificationId>,
    ) -> Result<Vec<AuthorizationGuard<Notification, Read>>, Error> {
        Ok(self
            .client
            .notification()
            .fetch_by_notification_ids(notification_ids)
            .await?
            .into_iter()
            .flat_map(TryInto::<Notification>::try_into)
            .map(Into::into)
            .map(AuthorizationGuard::<_, Create>::into_read)
            .collect_vec())
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

    async fn create_notification_settings(
        &self,
        actor: &User,
        notification_settings: &AuthorizationGuard<NotificationSettings, Create>,
    ) -> Result<(), Error> {
        notification_settings
            .try_create(actor, |settings| {
                self.client
                    .notification()
                    .upsert_notification_settings(settings)
            })?
            .await
            .map_err(Into::into)
    }

    async fn fetch_notification_settings(
        &self,
        recipient_id: Uuid,
    ) -> Result<Option<AuthorizationGuard<NotificationSettings, Read>>, Error> {
        Ok::<_, Error>(
            self.client
                .notification()
                .fetch_notification_settings(recipient_id)
                .await?
                .map(TryInto::<NotificationSettings>::try_into)
                .transpose()?
                .map(Into::into),
        )
    }

    async fn update_notification_settings(
        &self,
        actor: &User,
        notification_settings: AuthorizationGuard<NotificationSettings, Update>,
    ) -> Result<(), Error> {
        notification_settings
            .try_update(actor, |settings| {
                self.client
                    .notification()
                    .upsert_notification_settings(settings)
            })?
            .await
            .map_err(Into::into)
    }
}
