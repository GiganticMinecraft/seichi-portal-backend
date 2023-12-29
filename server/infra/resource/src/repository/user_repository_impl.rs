use async_trait::async_trait;
use domain::{
    repository::user_repository::UserRepository,
    user::models::{Role, User},
};
use errors::Error;
use uuid::Uuid;

use crate::{
    database::components::{DatabaseComponents, UserDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> UserRepository for Repository<Client> {
    async fn find_by(&self, uuid: Uuid) -> Result<Option<User>, Error> {
        self.client.user().find_by(uuid).await.map_err(Into::into)
    }

    async fn upsert_user(&self, user: &User) -> Result<(), Error> {
        self.client
            .user()
            .upsert_user(user)
            .await
            .map_err(Into::into)
    }

    async fn patch_user_role(&self, uuid: Uuid, role: Role) -> Result<(), Error> {
        self.client
            .user()
            .patch_user_role(uuid, role)
            .await
            .map_err(Into::into)
    }
}
