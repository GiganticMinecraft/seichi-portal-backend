use async_trait::async_trait;
use errors::Error;

use crate::{
    minecraft_ban::{MinecraftBan, MinecraftBanHistory},
    types::authorization_guard::{Allowed, Read},
};

#[async_trait]
pub trait MinecraftBanRepository: Send + Sync + 'static {
    /// 認可済みの対象ユーザーについてだけLiteBansを照会します。
    async fn list(
        &self,
        history: Allowed<MinecraftBanHistory, Read>,
    ) -> Result<Vec<MinecraftBan>, Error>;
}
