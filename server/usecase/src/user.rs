use domain::{
    repository::user_repository::UserRepository,
    user::models::{DiscordUserId, Role, User},
};
use errors::{usecase::UseCaseError, Error};
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

    pub async fn fetch_all_users(&self) -> Result<Vec<User>, Error> {
        self.repository.fetch_all_users().await
    }

    pub async fn fetch_user_by_xbox_token(&self, token: String) -> Result<Option<User>, Error> {
        let fetched_user = self.repository.fetch_user_by_xbox_token(token).await?;

        match fetched_user {
            Some(user) => {
                self.upsert_user(&user).await?;
                self.find_by(user.id).await
            }
            None => Ok(None),
        }
    }

    pub async fn start_user_session(
        &self,
        xbox_token: String,
        user: &User,
        expires: i32,
    ) -> Result<String, Error> {
        self.repository
            .start_user_session(xbox_token, user, expires)
            .await
    }

    pub async fn fetch_user_by_session_id(
        &self,
        session_id: String,
    ) -> Result<Option<User>, Error> {
        let fetched_user_uuid = self
            .repository
            .fetch_user_by_session_id(session_id)
            .await?
            .map(|user| user.id);

        match fetched_user_uuid {
            Some(uuid) => self.find_by(uuid).await,
            None => Ok(None),
        }
    }

    pub async fn end_user_session(&self, session_id: String) -> Result<(), Error> {
        self.repository.end_user_session(session_id).await
    }

    pub async fn link_discord_user(
        &self,
        discord_oauth_token: String,
        user: &User,
    ) -> Result<(), Error> {
        let discord_user_id = self
            .repository
            .fetch_discord_user_id_by_token(discord_oauth_token)
            .await?
            .ok_or(Error::from(UseCaseError::DiscordLinkFailed))?;

        self.repository
            .link_discord_user(&discord_user_id, user)
            .await
    }

    pub async fn unlink_discord_user(&self, user: &User) -> Result<(), Error> {
        self.repository.unlink_discord_user(user).await
    }

    pub async fn fetch_discord_user(
        &self,
        target_user_id: Uuid,
    ) -> Result<Option<DiscordUserId>, Error> {
        let user = self
            .repository
            .find_by(target_user_id)
            .await?
            .ok_or(Error::from(UseCaseError::UserNotFound))?;

        self.repository.fetch_discord_user_id(&user).await
    }
}
