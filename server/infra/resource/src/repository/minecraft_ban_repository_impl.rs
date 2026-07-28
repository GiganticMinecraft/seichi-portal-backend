use async_trait::async_trait;
use domain::{
    minecraft_ban::MinecraftBanHistory,
    repository::minecraft_ban_repository::MinecraftBanRepository,
    types::authorization_guard::{Allowed, AuthorizationGuard, Read},
};
use errors::Error;

use crate::{
    database::components::{DatabaseComponents, MinecraftBanDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> MinecraftBanRepository for Repository<Client> {
    async fn list_by_user_id(
        &self,
        history: Allowed<MinecraftBanHistory, Read>,
    ) -> Result<AuthorizationGuard<MinecraftBanHistory, Read>, Error> {
        Ok(MinecraftBanHistory::new(
            history.user_id(),
            self.client
                .minecraft_ban()
                .list_by_user_id(history.user_id())
                .await?,
        )?
        .into())
    }
}
