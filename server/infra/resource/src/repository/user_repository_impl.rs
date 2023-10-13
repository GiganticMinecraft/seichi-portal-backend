use async_trait::async_trait;
use domain::{repository::user_repository::UserRepository, user::models::User};
use errors::Error;

use crate::{
    database::components::{DatabaseComponents, UserDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> UserRepository for Repository<Client> {
    async fn upsert_user(&self, user: &User) -> Result<(), Error> {
        self.client
            .user()
            .upsert_user(user)
            .await
            .map_err(Into::into)
    }
}
