use async_trait::async_trait;
use errors::Error;
use mockall::automock;
use uuid::Uuid;

use crate::{
    account::models::{AccountUser, DiscordAccountLink, DiscordUser},
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Delete, Read, Update},
};

#[automock]
#[async_trait]
pub trait UserRepository: Send + Sync + 'static {
    async fn find_by(
        &self,
        uuid: Uuid,
    ) -> Result<Option<AuthorizationGuard<AccountUser, Read>>, Error>;
    async fn find_by_ids(
        &self,
        uuids: Vec<Uuid>,
    ) -> Result<Vec<AuthorizationGuard<AccountUser, Read>>, Error>;
    async fn upsert_user(&self, user: Allowed<AccountUser, Create>) -> Result<(), Error>;
    async fn patch_user_role(&self, user: Allowed<AccountUser, Update>) -> Result<(), Error>;
    async fn fetch_user_by_xbox_token(&self, token: String) -> Result<Option<AccountUser>, Error>;
    async fn fetch_all_users(&self) -> Result<Vec<AuthorizationGuard<AccountUser, Read>>, Error>;
    async fn start_user_session(
        &self,
        xbox_token: String,
        user: &AccountUser,
        expires: u32,
    ) -> Result<String, Error>;
    async fn fetch_user_by_session_id(
        &self,
        session_id: String,
    ) -> Result<Option<AccountUser>, Error>;
    async fn end_user_session(&self, session_id: String) -> Result<(), Error>;
    async fn link_discord_user(
        &self,
        link: Allowed<DiscordAccountLink, Update>,
    ) -> Result<(), Error>;
    async fn unlink_discord_user(
        &self,
        link: Allowed<DiscordAccountLink, Delete>,
    ) -> Result<(), Error>;
    async fn fetch_discord_user(
        &self,
        user: &Allowed<AccountUser, Read>,
    ) -> Result<Option<DiscordUser>, Error>;
    async fn fetch_discord_user_by_token(
        &self,
        token: String,
    ) -> Result<Option<DiscordUser>, Error>;
    async fn size(&self) -> Result<u32, Error>;
}
