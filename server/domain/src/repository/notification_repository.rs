use async_trait::async_trait;
use errors::Error;
use mockall::automock;
use uuid::Uuid;

use crate::{
    account::models::UserId,
    notification::models::{
        Notification, NotificationId, NotificationPagePosition, NotificationPreference,
    },
    pagination::{Page, PageRequest},
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Read, Update},
};

#[automock]
#[async_trait]
pub trait NotificationRepository: Send + Sync + 'static {
    async fn create_notification(
        &self,
        notification: Allowed<Notification, Create>,
    ) -> Result<(), Error>;
    async fn fetch_notification(
        &self,
        id: NotificationId,
    ) -> Result<Option<AuthorizationGuard<Notification, Read>>, Error>;
    async fn fetch_notifications(
        &self,
        recipient_id: UserId,
        request: PageRequest<NotificationPagePosition>,
    ) -> Result<Page<AuthorizationGuard<Notification, Read>, NotificationPagePosition>, Error>;
    async fn fetch_all_notifications(
        &self,
        recipient_id: UserId,
    ) -> Result<Vec<AuthorizationGuard<Notification, Read>>, Error>;
    async fn update_notification(
        &self,
        notification: Allowed<Notification, Update>,
    ) -> Result<(), Error>;
    async fn update_notifications(
        &self,
        notifications: Vec<Allowed<Notification, Update>>,
    ) -> Result<(), Error>;
    async fn create_notification_settings(
        &self,
        notification_settings: Allowed<NotificationPreference, Create>,
    ) -> Result<(), Error>;
    async fn fetch_notification_settings(
        &self,
        recipient_id: Uuid,
    ) -> Result<Option<AuthorizationGuard<NotificationPreference, Read>>, Error>;
    async fn update_notification_settings(
        &self,
        notification_settings: Allowed<NotificationPreference, Update>,
    ) -> Result<(), Error>;
}
