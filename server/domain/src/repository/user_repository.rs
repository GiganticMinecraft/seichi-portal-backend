use async_trait::async_trait;
use errors::Error;
use mockall::automock;
use uuid::Uuid;

use crate::{
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard_with_context::{Create, Read, Update},
    },
    user::models::{DiscordUserId, User},
};

#[automock]
#[async_trait]
pub trait UserRepository: Send + Sync + 'static {
    async fn find_by(&self, uuid: Uuid) -> Result<Option<AuthorizationGuard<User, Read>>, Error>;
    async fn upsert_user(
        &self,
        actor: &User,
        user: AuthorizationGuard<User, Create>,
    ) -> Result<(), Error>;
    async fn patch_user_role(
        &self,
        actor: &User,
        user: AuthorizationGuard<User, Update>,
    ) -> Result<(), Error>;
    async fn fetch_user_by_xbox_token(&self, token: String) -> Result<Option<User>, Error>;
    async fn fetch_all_users(&self) -> Result<Vec<AuthorizationGuard<User, Read>>, Error>;
    async fn start_user_session(
        &self,
        xbox_token: String,
        user: &User,
        expires: i32,
    ) -> Result<String, Error>;
    async fn fetch_user_by_session_id(&self, session_id: String) -> Result<Option<User>, Error>;
    async fn end_user_session(&self, session_id: String) -> Result<(), Error>;
    async fn link_discord_user(
        &self,
        actor: &User,
        discord_user_id: &DiscordUserId,
        user: AuthorizationGuard<User, Update>,
    ) -> Result<(), Error>;
    async fn unlink_discord_user(
        &self,
        actor: &User,
        user: AuthorizationGuard<User, Update>,
    ) -> Result<(), Error>;
    async fn fetch_discord_user_id(
        &self,
        actor: &User,
        user: AuthorizationGuard<User, Read>,
    ) -> Result<Option<DiscordUserId>, Error>;
    async fn fetch_discord_user_id_by_token(
        &self,
        token: String,
    ) -> Result<Option<DiscordUserId>, Error>;
}
