use async_trait::async_trait;
use errors::Error;

use crate::{
    minecraft_ban::MinecraftBanHistory,
    types::authorization_guard::{Allowed, AuthorizationGuard, Read},
};

#[async_trait]
pub trait MinecraftBanRepository: Send + Sync + 'static {
    async fn list_by_user_id(
        &self,
        history: Allowed<MinecraftBanHistory, Read>,
    ) -> Result<AuthorizationGuard<MinecraftBanHistory, Read>, Error>;
}
