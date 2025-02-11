use domain::{
    notification::models::{Notification, NotificationId, NotificationSettings},
    repository::{
        notification_repository::NotificationRepository, user_repository::UserRepository,
    },
    types::{authorization_guard::AuthorizationGuard, authorization_guard_with_context::Read},
    user::models::User,
};
use errors::{usecase::UseCaseError, Error};
use uuid::Uuid;

pub struct NotificationUseCase<
    'a,
    NotificationRepo: NotificationRepository,
    UserRepo: UserRepository,
> {
    pub repository: &'a NotificationRepo,
    pub user_repository: &'a UserRepo,
}

impl<R1: NotificationRepository, R2: UserRepository> NotificationUseCase<'_, R1, R2> {
    pub async fn fetch_notifications(
        &self,
        recipient_id: Uuid,
    ) -> Result<Vec<AuthorizationGuard<Notification, Read>>, Error> {
        self.repository.fetch_by_recipient_id(recipient_id).await
    }

    pub async fn update_notification_read_status(
        &self,
        actor: &User,
        notification_id_with_is_read: Vec<(NotificationId, bool)>,
    ) -> Result<Vec<AuthorizationGuard<Notification, Read>>, Error> {
        let (notification_id, is_read): (Vec<NotificationId>, Vec<bool>) =
            notification_id_with_is_read.into_iter().unzip();

        let notifications = self
            .repository
            .fetch_by_notification_ids(notification_id.to_owned())
            .await?;

        self.repository
            .update_read_status(
                actor,
                notifications
                    .into_iter()
                    .map(AuthorizationGuard::<_, Read>::into_update)
                    .zip(is_read.into_iter())
                    .collect::<Vec<_>>(),
            )
            .await?;

        self.repository
            .fetch_by_notification_ids(notification_id)
            .await
    }

    pub async fn fetch_notification_settings(
        &self,
        actor: User,
        target: Uuid,
    ) -> Result<NotificationSettings, Error> {
        let notification_settings = self.repository.fetch_notification_settings(target).await?;

        match notification_settings {
            Some(notification_settings) => notification_settings
                .try_into_read(&actor)
                .map_err(Into::into),
            None => {
                let target_user = self
                    .user_repository
                    .find_by(target)
                    .await?
                    .ok_or(Error::from(UseCaseError::UserNotFound))?;

                let notification_settings = NotificationSettings::new(target_user).into();

                self.repository
                    .create_notification_settings(&actor, &notification_settings)
                    .await?;

                Ok(notification_settings.into_read().try_into_read(&actor)?)
            }
        }
    }

    pub async fn update_notification_settings(
        &self,
        actor: &User,
        is_send_message_notification: bool,
    ) -> Result<(), Error> {
        let current_settings = self
            .repository
            .fetch_notification_settings(actor.id)
            .await?;

        let current_settings = match current_settings {
            Some(settings) => settings,
            None => {
                let notification_settings = NotificationSettings::new(actor.to_owned()).into();

                self.repository
                    .create_notification_settings(actor, &notification_settings)
                    .await?;

                notification_settings.into_read()
            }
        };

        let updated_notification_settings = current_settings.into_update().map(|settings| {
            settings.update_send_message_notification(is_send_message_notification)
        });

        self.repository
            .update_notification_settings(actor, updated_notification_settings)
            .await
    }
}
