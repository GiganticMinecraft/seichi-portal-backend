use domain::{
    auth::Actor,
    global_discord_webhook::GlobalDiscordWebhookSetting,
    repository::global_discord_webhook_repository::GlobalDiscordWebhookRepository,
    types::authorization_guard::{AuthorizationGuard, Update},
};
use errors::Error;

pub struct GlobalDiscordWebhookUseCase<'a, Repo: GlobalDiscordWebhookRepository> {
    pub repository: &'a Repo,
}

impl<Repo: GlobalDiscordWebhookRepository> GlobalDiscordWebhookUseCase<'_, Repo> {
    pub async fn get(&self, actor: &Actor) -> Result<GlobalDiscordWebhookSetting, Error> {
        self.repository
            .get()
            .await?
            .try_read(actor.clone())
            .map(|setting| setting.into_inner())
            .map_err(Into::into)
    }

    pub async fn update(
        &self,
        actor: &Actor,
        setting: GlobalDiscordWebhookSetting,
    ) -> Result<(), Error> {
        let setting = AuthorizationGuard::<_, Update>::from(setting).try_update(actor.clone())?;
        self.repository.update(setting).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{
        account::models::{AccountUser, Role, UserId},
        global_discord_webhook::ValidatedDiscordWebhookUrl,
        repository::global_discord_webhook_repository::MockGlobalDiscordWebhookRepository,
        types::authorization_guard::{Allowed, Read},
    };
    use errors::domain::DomainError;
    use types::non_empty_string::NonEmptyString;
    use uuid::Uuid;

    fn actor(role: Role) -> Actor {
        Actor::from(AccountUser::new(
            "actor".to_string(),
            UserId::from(Uuid::new_v4()),
            role,
        ))
    }

    fn enabled_setting() -> GlobalDiscordWebhookSetting {
        GlobalDiscordWebhookSetting::Enabled(
            ValidatedDiscordWebhookUrl::try_new(
                NonEmptyString::try_new("https://discord.com/api/webhooks/123/token".to_string())
                    .unwrap(),
            )
            .unwrap(),
        )
    }

    #[tokio::test]
    async fn administrator_can_read_enable_and_disable_without_reading_the_url_back() {
        let administrator = actor(Role::Administrator);
        let mut repository = MockGlobalDiscordWebhookRepository::new();
        repository
            .expect_get()
            .once()
            .returning(|| Ok(AuthorizationGuard::<_, Read>::from(enabled_setting())));
        repository
            .expect_update()
            .times(2)
            .withf(|setting: &Allowed<GlobalDiscordWebhookSetting, Update>| {
                matches!(
                    setting.value(),
                    GlobalDiscordWebhookSetting::Enabled(_) | GlobalDiscordWebhookSetting::Disabled
                )
            })
            .returning(|_| Ok(()));
        let usecase = GlobalDiscordWebhookUseCase {
            repository: &repository,
        };

        assert!(usecase.get(&administrator).await.unwrap().enabled());
        usecase
            .update(&administrator, enabled_setting())
            .await
            .unwrap();
        usecase
            .update(&administrator, GlobalDiscordWebhookSetting::Disabled)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn standard_user_cannot_read_or_update_the_setting() {
        let standard_user = actor(Role::StandardUser);
        let mut repository = MockGlobalDiscordWebhookRepository::new();
        repository
            .expect_get()
            .once()
            .returning(|| Ok(AuthorizationGuard::<_, Read>::from(enabled_setting())));
        repository.expect_update().never();
        let usecase = GlobalDiscordWebhookUseCase {
            repository: &repository,
        };

        assert_eq!(
            usecase.get(&standard_user).await,
            Err(DomainError::Forbidden.into())
        );
        assert_eq!(
            usecase
                .update(&standard_user, GlobalDiscordWebhookSetting::Disabled)
                .await,
            Err(DomainError::Forbidden.into())
        );
    }
}
