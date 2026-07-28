use async_trait::async_trait;
use domain::{
    minecraft_ban::{MinecraftBan, MinecraftBanHistory},
    repository::minecraft_ban_repository::MinecraftBanRepository,
    types::authorization_guard::{Allowed, Read},
};
use errors::Error;

use crate::{
    database::components::{DatabaseComponents, MinecraftBanDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> MinecraftBanRepository for Repository<Client> {
    async fn list(
        &self,
        history: Allowed<MinecraftBanHistory, Read>,
    ) -> Result<Vec<MinecraftBan>, Error> {
        self.client
            .minecraft_ban()
            .list_by_user_id(history.user_id())
            .await
            .map_err(Into::into)
    }
}
