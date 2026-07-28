use async_trait::async_trait;
use errors::Error;

use crate::{
    account::models::UserId,
    minecraft_ban::MinecraftBanHistory,
    types::authorization_guard::{AuthorizationGuard, Read},
};

#[async_trait]
pub trait MinecraftBanRepository: Send + Sync + 'static {
    async fn list_by_user_id(
        &self,
        user_id: UserId,
    ) -> Result<AuthorizationGuard<MinecraftBanHistory, Read>, Error>;
}
