use async_trait::async_trait;
use errors::Error;
use mockall::automock;
use uuid::Uuid;

use crate::user::models::{Role, User};

#[automock]
#[async_trait]
pub trait UserRepository: Send + Sync + 'static {
    async fn find_by(&self, uuid: Uuid) -> Result<Option<User>, Error>;
    async fn upsert_user(&self, user: &User) -> Result<(), Error>;
    async fn patch_user_role(&self, uuid: Uuid, role: Role) -> Result<(), Error>;
    async fn fetch_user_by_xbox_token(&self, token: String) -> Result<Option<User>, Error>;
    async fn start_user_session(
        &self,
        xbox_token: String,
        user: &User,
        expires: i32,
    ) -> Result<String, Error>;
    async fn fetch_user_by_session_id(&self, session_id: String) -> Result<Option<User>, Error>;
    async fn end_user_session(&self, session_id: String) -> Result<(), Error>;
}
