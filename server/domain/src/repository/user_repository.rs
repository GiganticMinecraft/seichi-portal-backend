use async_trait::async_trait;
use errors::Error;
use mockall::automock;
use uuid::Uuid;

use crate::{
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard::{Create, Read, Update},
    },
    user::models::{ActiveUser, DiscordUser},
};

#[automock]
#[async_trait]
pub trait UserRepository: Send + Sync + 'static {
    async fn find_by(
        &self,
        uuid: Uuid,
    ) -> Result<Option<AuthorizationGuard<ActiveUser, Read>>, Error>;
    async fn find_by_ids(
        &self,
        uuids: Vec<Uuid>,
    ) -> Result<Vec<AuthorizationGuard<ActiveUser, Read>>, Error>;
    async fn upsert_user(
        &self,
        actor: &ActiveUser,
        user: AuthorizationGuard<ActiveUser, Create>,
    ) -> Result<(), Error>;
    async fn patch_user_role(
        &self,
        actor: &ActiveUser,
        user: AuthorizationGuard<ActiveUser, Update>,
    ) -> Result<(), Error>;
    async fn fetch_user_by_xbox_token(&self, token: String) -> Result<Option<ActiveUser>, Error>;
    async fn fetch_all_users(&self) -> Result<Vec<AuthorizationGuard<ActiveUser, Read>>, Error>;
    async fn start_user_session(
        &self,
        xbox_token: String,
        user: &ActiveUser,
        expires: u32,
    ) -> Result<String, Error>;
    async fn fetch_user_by_session_id(
        &self,
        session_id: String,
    ) -> Result<Option<ActiveUser>, Error>;
    async fn end_user_session(&self, session_id: String) -> Result<(), Error>;
    async fn link_discord_user(
        &self,
        actor: &ActiveUser,
        discord_user: &DiscordUser,
        user: AuthorizationGuard<ActiveUser, Update>,
    ) -> Result<(), Error>;
    async fn unlink_discord_user(
        &self,
        actor: &ActiveUser,
        user: AuthorizationGuard<ActiveUser, Update>,
    ) -> Result<(), Error>;
    async fn fetch_discord_user(
        &self,
        actor: &ActiveUser,
        user: &AuthorizationGuard<ActiveUser, Read>,
    ) -> Result<Option<DiscordUser>, Error>;
    async fn fetch_discord_user_by_token(
        &self,
        token: String,
    ) -> Result<Option<DiscordUser>, Error>;
    async fn size(&self) -> Result<u32, Error>;
}
