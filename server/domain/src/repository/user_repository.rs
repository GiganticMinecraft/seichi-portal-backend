use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::user::models::User;

#[automock]
#[async_trait]
pub trait UserRepository: Send + Sync + 'static {
    async fn upsert_user(&self, user: &User) -> Result<(), Error>;
}
