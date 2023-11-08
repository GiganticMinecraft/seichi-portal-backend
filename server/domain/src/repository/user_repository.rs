use async_trait::async_trait;
use errors::Error;
use mockall::automock;
use uuid::Uuid;

use crate::user::models::{Role, User};

#[automock]
#[async_trait]
pub trait UserRepository: Send + Sync + 'static {
    async fn upsert_user(&self, user: &User) -> Result<(), Error>;
    async fn patch_user_role(&self, uuid: Uuid, role: Role) -> Result<(), Error>;
}
