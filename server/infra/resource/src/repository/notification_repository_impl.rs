use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::account::models::UserId;
use domain::{
    notification::models::{
        MarkAllNotificationsAsRead, Notification, NotificationId, NotificationPagePosition,
        NotificationPreference,
    },
    pagination::{Page, PageRequest},
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
    async fn create_notification(
        &self,
        notification: Allowed<Notification, Create>,
    ) -> Result<(), Error> {
        self.client
            .notification()
            .create_notification(notification.value())
            .await
            .map_err(Into::into)
    }

    async fn fetch_notification(
        &self,
        id: NotificationId,
    ) -> Result<Option<AuthorizationGuard<Notification, Read>>, Error> {
        self.client
            .notification()
            .fetch_notification(id)
            .await?
            .map(TryInto::<Notification>::try_into)
            .transpose()
            .map(|notification| notification.map(Into::into))
    }

    async fn fetch_notifications(
        &self,
        recipient_id: UserId,
        request: PageRequest<NotificationPagePosition>,
    ) -> Result<Page<AuthorizationGuard<Notification, Read>, NotificationPagePosition>, Error> {
        let page = self
            .client
            .notification()
            .fetch_notifications(recipient_id, request)
            .await?;
        let (records, next) = page.into_parts();
        let notifications = records
            .into_iter()
            .map(TryInto::<Notification>::try_into)
            .map(|notification| notification.map(Into::into))
            .collect::<Result<Vec<_>, Error>>()?;

        Ok(Page::new(notifications, next))
    }

    async fn update_notification(
        &self,
        notification: Allowed<Notification, Update>,
    ) -> Result<(), Error> {
        self.client
            .notification()
            .update_notification(notification.value())
            .await
            .map_err(Into::into)
    }

    async fn mark_all_notifications_as_read(
        &self,
        operation: Allowed<MarkAllNotificationsAsRead, Update>,
        read_at: DateTime<Utc>,
    ) -> Result<(), Error> {
        self.client
            .notification()
            .mark_all_notifications_as_read(operation.value(), read_at)
            .await
            .map_err(Into::into)
    }

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
