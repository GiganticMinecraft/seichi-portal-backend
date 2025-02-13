use domain::{
    repository::user_repository::UserRepository,
    user::models::{Role, User},
};
use errors::{usecase::UseCaseError, Error};
use uuid::Uuid;

use crate::dto::UserDto;

pub struct UserUseCase<'a, UserRepo: UserRepository> {
    pub repository: &'a UserRepo,
}

impl<R: UserRepository> UserUseCase<'_, R> {
    pub async fn find_by(&self, actor: &User, uuid: Uuid) -> Result<Option<User>, Error> {
        self.repository
            .find_by(uuid)
            .await?
            .map(|guard| guard.try_into_read(actor))
            .transpose()
            .map_err(Into::into)
    }

    pub async fn upsert_user(&self, actor: &User, upsert_target: User) -> Result<(), Error> {
        self.repository
            .upsert_user(actor, upsert_target.into())
            .await
    }

    pub async fn patch_user_role(&self, actor: &User, uuid: Uuid, role: Role) -> Result<(), Error> {
        let current_user_guard = self
            .repository
            .find_by(uuid)
            .await?
            .ok_or(Error::from(UseCaseError::UserNotFound))?;

        let current_user = current_user_guard.try_into_read(actor)?;
        let new_role_user = User {
            role,
            ..current_user
        };

        self.repository
            .patch_user_role(actor, new_role_user.into())
            .await
    }

    pub async fn fetch_all_users(&self, actor: &User) -> Result<Vec<User>, Error> {
        self.repository
            .fetch_all_users()
            .await?
            .into_iter()
            .map(|guard| guard.try_into_read(actor))
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub async fn fetch_user_by_xbox_token(&self, token: String) -> Result<Option<User>, Error> {
        let fetched_user = self.repository.fetch_user_by_xbox_token(token).await?;

        match fetched_user {
            Some(user) => {
                let guard = user.to_owned().into();
                self.repository.upsert_user(&user, guard).await?;
                // NOTE: リクエスト時点では token しかわからないので
                //  token で検索したユーザーが操作者であるとする
                self.find_by(&user, user.id).await
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
        let user = self.repository.fetch_user_by_session_id(session_id).await?;

        match user {
            Some(user) => self.find_by(&user, user.id).await,
            None => Ok(None),
        }
    }

    pub async fn end_user_session(&self, session_id: String) -> Result<(), Error> {
        self.repository.end_user_session(session_id).await
    }

    pub async fn link_discord_user(
        &self,
        discord_oauth_token: String,
        user: User,
    ) -> Result<(), Error> {
        let discord_user = self
            .repository
            .fetch_discord_user_by_token(discord_oauth_token)
            .await?
            .ok_or(Error::from(UseCaseError::DiscordLinkFailed))?;

        self.repository
            .link_discord_user(&user.to_owned(), &discord_user, user.into())
            .await
    }

    pub async fn unlink_discord_user(&self, user: User) -> Result<(), Error> {
        self.repository
            .unlink_discord_user(&user.to_owned(), user.into())
            .await
    }

    pub async fn fetch_user_information(
        &self,
        actor: &User,
        target_user_id: Uuid,
    ) -> Result<UserDto, Error> {
        let guard = self
            .repository
            .find_by(target_user_id)
            .await?
            .ok_or(Error::from(UseCaseError::UserNotFound))?;

        let discord_user = self.repository.fetch_discord_user(actor, &guard).await?;

        let user = guard.try_into_read(actor)?;

        Ok(UserDto { user, discord_user })
    }
}
