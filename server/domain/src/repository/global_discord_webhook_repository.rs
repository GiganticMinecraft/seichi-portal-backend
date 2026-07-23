use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    global_discord_webhook::GlobalDiscordWebhookSetting,
    types::authorization_guard::{Allowed, AuthorizationGuard, Read, Update},
};

#[automock]
#[async_trait]
pub trait GlobalDiscordWebhookRepository: Send + Sync + 'static {
    async fn get(&self) -> Result<AuthorizationGuard<GlobalDiscordWebhookSetting, Read>, Error>;

    async fn update(
        &self,
        setting: Allowed<GlobalDiscordWebhookSetting, Update>,
    ) -> Result<(), Error>;
}
