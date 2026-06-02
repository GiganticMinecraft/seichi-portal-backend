use async_trait::async_trait;
use domain::{
    notification::models::NotificationPreference,
    repository::notification_repository::NotificationRepository,
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Read, Update},
};
use errors::Error;
use uuid::Uuid;

use crate::{
    database::components::{DatabaseComponents, NotificationDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> NotificationRepository for Repository<Client> {
    async fn create_notification_settings(
        &self,
        notification_settings: Allowed<NotificationPreference, Create>,
    ) -> Result<(), Error> {
        self.client
            .notification()
            .upsert_notification_settings(notification_settings.value())
            .await
            .map_err(Into::into)
    }

    async fn fetch_notification_settings(
        &self,
        recipient_id: Uuid,
    ) -> Result<Option<AuthorizationGuard<NotificationPreference, Read>>, Error> {
        Ok::<_, Error>(
            self.client
                .notification()
                .fetch_notification_settings(recipient_id)
                .await?
                .map(TryInto::<NotificationPreference>::try_into)
                .transpose()?
                .map(Into::into),
        )
    }

    async fn update_notification_settings(
        &self,
        notification_settings: Allowed<NotificationPreference, Update>,
    ) -> Result<(), Error> {
        self.client
            .notification()
            .upsert_notification_settings(notification_settings.value())
            .await
            .map_err(Into::into)
    }
}
