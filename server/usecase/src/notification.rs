use domain::{
    notification::models::NotificationSettings,
    repository::{
        notification_repository::NotificationRepository, user_repository::UserRepository,
    },
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

                let notification_settings =
                    NotificationSettings::new(target_user.try_into_read(&actor)?).into();

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
