use async_trait::async_trait;
use errors::Error;
use uuid::Uuid;

use crate::{
    minecraft_ban::MinecraftBanHistory,
    types::authorization_guard::{AuthorizationGuard, Read},
};

#[async_trait]
pub trait MinecraftBanRepository: Send + Sync + 'static {
    async fn list_by_user_id(
        &self,
        user_id: Uuid,
    ) -> Result<AuthorizationGuard<MinecraftBanHistory, Read>, Error>;
}
