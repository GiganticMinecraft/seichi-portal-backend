use domain::types::authorization_guard::{AuthorizationGuard, Create, Read};
use domain::{
    account::models::AccountUser,
    auth::Actor,
    notification::models::NotificationPreference,
    repository::{
        notification_repository::NotificationRepository, user_repository::UserRepository,
    },
};
use errors::{Error, usecase::UseCaseError};
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
        actor: AccountUser,
        target: Uuid,
    ) -> Result<NotificationPreference, Error> {
        let actor_user = Actor::from(actor);
        let notification_settings = self.repository.fetch_notification_settings(target).await?;

        match notification_settings {
            Some(notification_settings) => notification_settings
                .try_read(actor_user.clone())
                .map(|settings| settings.into_inner())
                .map_err(Into::into),
            None => {
                let target_user = self
                    .user_repository
                    .find_by(target)
                    .await?
                    .ok_or(Error::from(UseCaseError::UserNotFound))?;

                let target_user = target_user.try_read(actor_user.clone())?.into_inner();
                let notification_settings: AuthorizationGuard<NotificationPreference, Create> =
                    NotificationPreference::new(*target_user.id()).into();

                Ok(notification_settings
                    .into_read()
                    .try_read(actor_user.clone())?
                    .into_inner())
            }
        }
    }

    pub async fn update_notification_settings(
        &self,
        actor: &AccountUser,
        is_send_message_notification: Option<bool>,
    ) -> Result<(), Error> {
        // NOTE: Discord への通知設定は、Discord への連携がすでに行われていなければならない
        let user = self
            .user_repository
            .find_by(actor.id().into_inner())
            .await?
            .ok_or(UseCaseError::UserNotFound)?
            .try_read(Actor::from(actor.clone()))?;

        let discord_user = self.user_repository.fetch_discord_user(&user).await?;

        if discord_user.is_none() {
            return Err(Error::from(UseCaseError::DiscordNotLinked));
        }

        let current_settings = self
            .repository
            .fetch_notification_settings(actor.id().into_inner())
            .await?;

        let current_settings = match current_settings {
            Some(settings) => settings,
            None => {
                let preference = NotificationPreference::new(*actor.id());

                self.repository
                    .create_notification_settings(
                        AuthorizationGuard::<_, Create>::from(preference.clone())
                            .try_create(Actor::from(actor.clone()))?,
                    )
                    .await?;

                AuthorizationGuard::<_, Read>::from(preference)
            }
        };

        match is_send_message_notification {
            Some(is_send_message_notification) => {
                let updated_notification_settings = current_settings
                    .into_update()
                    .map(|settings| {
                        settings.update_send_message_notification(is_send_message_notification)
                    })
                    .try_update(Actor::from(actor.clone()))?;

                self.repository
                    .update_notification_settings(updated_notification_settings)
                    .await
            }
            None => Ok(()),
        }
    }
}
