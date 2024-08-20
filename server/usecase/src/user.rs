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

    pub async fn fetch_user_by_xbox_token(&self, token: String) -> Result<Option<User>, Error> {
        self.repository.fetch_user_by_xbox_token(token).await
    }

    pub async fn start_user_session(
        &self,
        xbox_token: String,
        user: &User,
    ) -> Result<String, Error> {
        self.repository.start_user_session(xbox_token, user).await
    }

    pub async fn fetch_user_by_session_id(
        &self,
        session_id: String,
    ) -> Result<Option<User>, Error> {
        self.repository.fetch_user_by_session_id(session_id).await
    }

    pub async fn update_user_session(&self, session_id: String) -> Result<(), Error> {
        self.repository.update_user_session(session_id).await
    }

    pub async fn end_user_session(&self, session_id: String) -> Result<(), Error> {
        self.repository.end_user_session(session_id).await
    }
}
