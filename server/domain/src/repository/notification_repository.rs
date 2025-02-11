use async_trait::async_trait;
use errors::Error;
use uuid::Uuid;

use crate::{
    notification::models::{Notification, NotificationId, NotificationSettings},
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard_with_context::{Create, Read, Update},
    },
    user::models::User,
};

#[async_trait]
pub trait NotificationRepository: Send + Sync + 'static {
    async fn create(&self, notification: &Notification) -> Result<(), Error>;
    async fn fetch_by_recipient_id(
        &self,
        recipient_id: Uuid,
    ) -> Result<Vec<AuthorizationGuard<Notification, Read>>, Error>;
    async fn fetch_by_notification_ids(
        &self,
        notification_ids: Vec<NotificationId>,
    ) -> Result<Vec<AuthorizationGuard<Notification, Read>>, Error>;
    async fn update_read_status(
        &self,
        actor: &User,
        notifications: Vec<(AuthorizationGuard<Notification, Update>, bool)>,
    ) -> Result<(), Error>;
    async fn create_notification_settings(
        &self,
        actor: &User,
        notification_settings: &AuthorizationGuard<NotificationSettings, Create>,
    ) -> Result<(), Error>;
    async fn fetch_notification_settings(
        &self,
        recipient_id: Uuid,
    ) -> Result<Option<AuthorizationGuard<NotificationSettings, Read>>, Error>;
    async fn update_notification_settings(
        &self,
        actor: &User,
        notification_settings: AuthorizationGuard<NotificationSettings, Update>,
    ) -> Result<(), Error>;
}
