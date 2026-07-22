use async_trait::async_trait;
use domain::{
    global_discord_webhook::GlobalDiscordWebhookSetting,
    repository::global_discord_webhook_repository::GlobalDiscordWebhookRepository,
    types::authorization_guard::{Allowed, AuthorizationGuard, Read, Update},
};
use errors::Error;
use types::non_empty_string::NonEmptyString;

use crate::{database::connection::ConnectionPool, repository::Repository};

#[async_trait]
impl GlobalDiscordWebhookRepository for Repository<ConnectionPool> {
    async fn get(&self) -> Result<AuthorizationGuard<GlobalDiscordWebhookSetting, Read>, Error> {
        let url = self.client.fetch_global_discord_webhook_url().await?;
        let url = url.map(NonEmptyString::try_new).transpose()?;
        let setting = GlobalDiscordWebhookSetting::from_optional_url(url)?;

        Ok(AuthorizationGuard::from(setting))
    }

    async fn update(
        &self,
        setting: Allowed<GlobalDiscordWebhookSetting, Update>,
    ) -> Result<(), Error> {
        self.client
            .update_global_discord_webhook_url(setting.value().url().map(|url| url.as_str()))
            .await
            .map_err(Into::into)
    }
}
