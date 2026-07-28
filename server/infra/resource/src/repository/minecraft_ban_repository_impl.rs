use async_trait::async_trait;
use domain::{
    minecraft_ban::MinecraftBanHistory,
    repository::minecraft_ban_repository::MinecraftBanRepository,
    types::authorization_guard::{AuthorizationGuard, Read},
};
use errors::Error;
use uuid::Uuid;

use crate::{
    database::components::{DatabaseComponents, MinecraftBanDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> MinecraftBanRepository for Repository<Client> {
    async fn list_by_user_id(
        &self,
        user_id: Uuid,
    ) -> Result<AuthorizationGuard<MinecraftBanHistory, Read>, Error> {
        Ok(MinecraftBanHistory::new(
            user_id.into(),
            self.client
                .minecraft_ban()
                .list_by_user_id(user_id.into())
                .await?,
        )?
        .into())
    }
}
