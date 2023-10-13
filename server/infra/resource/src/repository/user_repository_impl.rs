use crate::database::components::UserDatabase;
use crate::{database::components::DatabaseComponents, repository::Repository};
use async_trait::async_trait;
use domain::repository::user_repository::UserRepository;
use domain::user::models::User;
use errors::Error;

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
