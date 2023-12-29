use domain::{
    repository::user_repository::UserRepository,
    user::models::{Role, User},
};
use errors::Error;
use uuid::Uuid;

pub struct UserUseCase<'a, UserRepo: UserRepository> {
    pub repository: &'a UserRepo,
}

impl<R: UserRepository> UserUseCase<'_, R> {
    pub async fn find_by(&self, uuid: Uuid) -> Result<Option<User>, Error> {
        self.repository.find_by(uuid).await
    }

    pub async fn upsert_user(&self, user: &User) -> Result<(), Error> {
        self.repository.upsert_user(user).await
    }

    pub async fn patch_user_role(&self, uuid: Uuid, role: Role) -> Result<(), Error> {
        self.repository.patch_user_role(uuid, role).await
    }
}
