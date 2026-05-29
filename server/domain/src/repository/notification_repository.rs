use async_trait::async_trait;
use errors::Error;
use mockall::automock;
use uuid::Uuid;

use crate::{
    notification::models::NotificationPreference,
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard::{Create, Read, Update},
    },
    user::models::ActiveUser,
};

#[automock]
#[async_trait]
pub trait NotificationRepository: Send + Sync + 'static {
    async fn create_notification_settings(
        &self,
        actor: &ActiveUser,
        notification_settings: &AuthorizationGuard<NotificationPreference, Create>,
    ) -> Result<(), Error>;
    async fn fetch_notification_settings(
        &self,
        recipient_id: Uuid,
    ) -> Result<Option<AuthorizationGuard<NotificationPreference, Read>>, Error>;
    async fn update_notification_settings(
        &self,
        actor: &ActiveUser,
        notification_settings: AuthorizationGuard<NotificationPreference, Update>,
    ) -> Result<(), Error>;
}
