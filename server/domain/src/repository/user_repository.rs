use async_trait::async_trait;
use errors::Error;
use mockall::automock;
use uuid::Uuid;

use crate::{
    account::models::{
        AccountUser, DiscordAccountLink, DiscordUser, UserGroup, UserGroupId, UserPagePosition,
    },
    pagination::{Page, PageRequest},
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
    async fn create_user_group(&self, group: Allowed<UserGroup, Create>) -> Result<(), Error>;
    async fn update_user_group(&self, group: Allowed<UserGroup, Update>) -> Result<(), Error>;
    async fn delete_user_group(&self, group: Allowed<UserGroup, Delete>) -> Result<(), Error>;
    async fn find_user_group(
        &self,
        group_id: UserGroupId,
    ) -> Result<Option<AuthorizationGuard<UserGroup, Read>>, Error>;
    async fn fetch_user_groups(&self) -> Result<Vec<AuthorizationGuard<UserGroup, Read>>, Error>;
    async fn fetch_users_by_group(
        &self,
        group: Allowed<UserGroup, Read>,
    ) -> Result<Vec<AuthorizationGuard<AccountUser, Read>>, Error>;
    async fn add_user_to_group(
        &self,
        group: Allowed<UserGroup, Update>,
        user: Allowed<AccountUser, Update>,
    ) -> Result<(), Error>;
    async fn remove_user_from_group(
        &self,
        group: Allowed<UserGroup, Update>,
        user: Allowed<AccountUser, Update>,
    ) -> Result<(), Error>;
    async fn fetch_user_by_xbox_token(&self, token: String) -> Result<Option<AccountUser>, Error>;
    async fn fetch_all_users(&self) -> Result<Vec<AuthorizationGuard<AccountUser, Read>>, Error>;
    async fn fetch_users_page(
        &self,
        request: PageRequest<UserPagePosition>,
    ) -> Result<Page<AuthorizationGuard<AccountUser, Read>, UserPagePosition>, Error>;
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
