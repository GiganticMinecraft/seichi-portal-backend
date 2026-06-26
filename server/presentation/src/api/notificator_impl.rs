use async_trait::async_trait;
use domain::notification::models::{NotificationContent, NotificationPreference, NotificationType};
use domain::notification::notificator::Notificator;
use domain::repository::Repositories;
use domain::repository::user_repository::UserRepository;
use domain::{account::models::UserId, auth::Actor};
use errors::Error;
use errors::usecase::UseCaseError::UserNotFound;
use resource::outgoing::connection::ConnectionPool;

pub struct DiscordNotificator<R: Repositories> {
    discord_connection: ConnectionPool,
    repositories: R,
}

impl<R: Repositories> DiscordNotificator<R> {
    pub fn new(discord_connection: ConnectionPool, repositories: R) -> Self {
        Self {
            discord_connection,
            repositories,
        }
    }
}

#[async_trait]
impl<R: Repositories> Notificator for DiscordNotificator<R> {
    async fn notify(
        &self,
        recipient: UserId,
        notification_type: NotificationType,
        notification_preference: &NotificationPreference,
        content: &NotificationContent,
    ) -> Result<(), Error> {
        if !notification_preference.is_enabled(&notification_type) {
            return Ok(());
        }

        let user_guard = self
            .repositories
            .user_repository()
            .find_by(recipient.into_inner())
            .await?
            .ok_or(Error::from(UserNotFound))?;

        let user = user_guard.try_read(Actor::System)?;

        let discord_user = self
            .repositories
            .user_repository()
            .fetch_discord_user(&user)
            .await?;

        if let Some(discord_user) = discord_user {
            self.discord_connection
                .send_direct_message(discord_user.id().to_owned(), content.to_message())
                .await?;
        }

        Ok(())
    }
}
