use async_trait::async_trait;
use errors::Error;
use mockall::automock;
use uuid::Uuid;

use crate::{
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Delete, Read, Update},
    user::models::{ActiveUser, AnswerSubmissionRestriction, DiscordAccountLink, DiscordUser},
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
    async fn upsert_user(&self, user: Allowed<ActiveUser, Create>) -> Result<(), Error>;
    async fn patch_user_role(&self, user: Allowed<ActiveUser, Update>) -> Result<(), Error>;
    async fn fetch_active_answer_submission_restriction(
        &self,
        user_id: Uuid,
    ) -> Result<Option<AnswerSubmissionRestriction>, Error>;
    async fn restrict_answer_submission(
        &self,
        restriction: Allowed<AnswerSubmissionRestriction, Create>,
    ) -> Result<(), Error>;
    async fn lift_answer_submission_restriction(
        &self,
        user_id: Uuid,
        actor: &ActiveUser,
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
        link: Allowed<DiscordAccountLink, Update>,
    ) -> Result<(), Error>;
    async fn unlink_discord_user(
        &self,
        link: Allowed<DiscordAccountLink, Delete>,
    ) -> Result<(), Error>;
    async fn fetch_discord_user(
        &self,
        user: &Allowed<ActiveUser, Read>,
    ) -> Result<Option<DiscordUser>, Error>;
    async fn fetch_discord_user_by_token(
        &self,
        token: String,
    ) -> Result<Option<DiscordUser>, Error>;
    async fn size(&self) -> Result<u32, Error>;
}
