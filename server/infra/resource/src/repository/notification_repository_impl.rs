use async_trait::async_trait;
use domain::{
    account::models::UserId,
    notification::models::{
        Notification, NotificationId, NotificationPagePosition, NotificationPreference,
    },
    pagination::{Page, PageRequest},
    repository::notification_repository::NotificationRepository,
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Read, Update},
};
use errors::Error;
use uuid::Uuid;

use crate::{
    database::components::{DatabaseComponents, NotificationDatabase},
    records::NotificationRecord,
    repository::Repository,
};

fn into_notification_guards(
    records: Vec<NotificationRecord>,
) -> Result<Vec<AuthorizationGuard<Notification, Read>>, Error> {
    records
        .into_iter()
        .map(TryInto::<Notification>::try_into)
        .map(|notification| notification.map(Into::into))
        .collect()
}

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
        let notifications = into_notification_guards(records)?;

        Ok(Page::new(notifications, next))
    }

    async fn fetch_all_notifications(
        &self,
        recipient_id: UserId,
    ) -> Result<Vec<AuthorizationGuard<Notification, Read>>, Error> {
        let records = self
            .client
            .notification()
            .fetch_all_notifications(recipient_id)
            .await?;

        into_notification_guards(records)
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

    async fn update_notifications(
        &self,
        notifications: Vec<Allowed<Notification, Update>>,
    ) -> Result<(), Error> {
        let notifications = notifications
            .into_iter()
            .map(Allowed::into_inner)
            .collect::<Vec<_>>();

        self.client
            .notification()
            .update_notifications(&notifications)
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
