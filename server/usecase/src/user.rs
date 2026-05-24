use domain::{
    repository::user_repository::UserRepository,
    user::models::{ActiveUser, Role, User},
};
use errors::{Error, usecase::UseCaseError};
use uuid::Uuid;

use crate::models::UserProfile;

pub struct UserUseCase<'a, UserRepo: UserRepository> {
    pub repository: &'a UserRepo,
}

impl<R: UserRepository> UserUseCase<'_, R> {
    pub async fn find_by(&self, actor: &ActiveUser, uuid: Uuid) -> Result<ActiveUser, Error> {
        let actor = User::ActiveUser(actor.clone());
        self.repository
            .find_by(uuid)
            .await?
            .map(|guard| guard.try_into_read(&actor))
            .transpose()?
            .ok_or(Error::from(UseCaseError::UserNotFound))
    }

    pub async fn upsert_user(
        &self,
        actor: &ActiveUser,
        upsert_target: ActiveUser,
    ) -> Result<(), Error> {
        let actor = User::ActiveUser(actor.clone());
        self.repository
            .upsert_user(&actor, upsert_target.into())
            .await
    }

    pub async fn patch_user_role(
        &self,
        actor: &ActiveUser,
        uuid: Uuid,
        role: Role,
    ) -> Result<ActiveUser, Error> {
        let actor = User::ActiveUser(actor.clone());
        let current_user_guard = self
            .repository
            .find_by(uuid)
            .await?
            .ok_or(Error::from(UseCaseError::UserNotFound))?;

        let current_user = current_user_guard.try_into_read(&actor)?;
        let new_role_user =
            ActiveUser::new(current_user.name().to_owned(), *current_user.id(), role);

        self.repository
            .patch_user_role(&actor, new_role_user.into())
            .await?;

        let updated_user_guard = self
            .repository
            .find_by(uuid)
            .await?
            .ok_or(Error::from(UseCaseError::UserNotFound))?;

        updated_user_guard.try_into_read(&actor).map_err(Into::into)
    }

    pub async fn fetch_all_users(&self, actor: &ActiveUser) -> Result<Vec<ActiveUser>, Error> {
        let actor = User::ActiveUser(actor.clone());
        self.repository
            .fetch_all_users()
            .await?
            .into_iter()
            .map(|guard| guard.try_into_read(&actor))
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub async fn fetch_user_by_xbox_token(
        &self,
        token: String,
    ) -> Result<Option<ActiveUser>, Error> {
        let fetched_user = self.repository.fetch_user_by_xbox_token(token).await?;

        match fetched_user {
            Some(user) => {
                let guard = user.to_owned().into();
                let actor = User::ActiveUser(user.clone());
                self.repository.upsert_user(&actor, guard).await?;
                // NOTE: リクエスト時点では token しかわからないので
                //  token で検索したユーザーが操作者であるとする
                self.repository
                    .find_by(user.id().into_inner())
                    .await?
                    .map(|guard| guard.try_into_read(&actor))
                    .transpose()
                    .map_err(Into::into)
            }
            None => Ok(None),
        }
    }

    pub async fn start_user_session(
        &self,
        xbox_token: String,
        user: &ActiveUser,
        expires: u32,
    ) -> Result<String, Error> {
        self.repository
            .start_user_session(xbox_token, user, expires)
            .await
    }

    pub async fn fetch_user_by_session_id(
        &self,
        session_id: String,
    ) -> Result<Option<ActiveUser>, Error> {
        self.repository.fetch_user_by_session_id(session_id).await
    }

    pub async fn end_user_session(&self, session_id: String) -> Result<(), Error> {
        self.repository.end_user_session(session_id).await
    }

    pub async fn link_discord_user(
        &self,
        discord_oauth_token: String,
        user: ActiveUser,
    ) -> Result<(), Error> {
        let discord_user = self
            .repository
            .fetch_discord_user_by_token(discord_oauth_token)
            .await?
            .ok_or(Error::from(UseCaseError::DiscordLinkFailed))?;

        self.repository
            .link_discord_user(&User::ActiveUser(user.clone()), &discord_user, user.into())
            .await
    }

    pub async fn unlink_discord_user(&self, user: ActiveUser) -> Result<(), Error> {
        self.repository
            .unlink_discord_user(&User::ActiveUser(user.clone()), user.into())
            .await
    }

    pub async fn fetch_user_information(
        &self,
        actor: &ActiveUser,
        target_user_id: Uuid,
    ) -> Result<UserProfile, Error> {
        let guard = self
            .repository
            .find_by(target_user_id)
            .await?
            .ok_or(Error::from(UseCaseError::UserNotFound))?;

        let actor = User::ActiveUser(actor.clone());
        let discord_user = self.repository.fetch_discord_user(&actor, &guard).await?;

        let user = guard.try_into_read(&actor)?;

        Ok(UserProfile { user, discord_user })
    }
}
