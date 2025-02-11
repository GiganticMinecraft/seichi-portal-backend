use async_trait::async_trait;
use domain::{
    notification::models::NotificationSettings,
    repository::notification_repository::NotificationRepository,
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard_with_context::{Create, Read, Update},
    },
    user::models::User,
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
