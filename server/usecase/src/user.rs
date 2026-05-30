use domain::{
    repository::user_repository::UserRepository,
    types::authorization_guard::{AuthorizationGuard, Create, Update},
    user::models::{ActiveUser, Actor, Role},
};
use errors::{Error, usecase::UseCaseError};
use uuid::Uuid;

use crate::models::UserProfile;

pub struct UserUseCase<'a, UserRepo: UserRepository> {
    pub repository: &'a UserRepo,
}

impl<R: UserRepository> UserUseCase<'_, R> {
    pub async fn find_by(&self, actor: &ActiveUser, uuid: Uuid) -> Result<ActiveUser, Error> {
        let actor_ref = Actor::from(actor.clone());
        self.repository
            .find_by(uuid)
            .await?
            .map(|guard| {
                guard
                    .try_read(actor_ref.clone())
                    .map(|user| user.into_inner())
            })
            .transpose()?
            .ok_or(Error::from(UseCaseError::UserNotFound))
    }

    pub async fn upsert_user(
        &self,
        actor: &ActiveUser,
        upsert_target: ActiveUser,
    ) -> Result<(), Error> {
        self.repository
            .upsert_user(
                AuthorizationGuard::<_, Create>::from(upsert_target)
                    .try_create(Actor::from(actor.clone()))?,
            )
            .await
    }

    pub async fn patch_user_role(
        &self,
        actor: &ActiveUser,
        uuid: Uuid,
        role: Role,
    ) -> Result<ActiveUser, Error> {
        let actor_ref = Actor::from(actor.clone());
        let current_user_guard = self
            .repository
            .find_by(uuid)
            .await?
            .ok_or(Error::from(UseCaseError::UserNotFound))?;

        let current_user = current_user_guard.try_read(actor_ref.clone())?.into_inner();
        let new_role_user =
            ActiveUser::new(current_user.name().to_owned(), *current_user.id(), role);

        self.repository
            .patch_user_role(
                AuthorizationGuard::<_, Update>::from(new_role_user)
                    .try_update(actor_ref.clone())?,
            )
            .await?;

        let updated_user_guard = self
            .repository
            .find_by(uuid)
            .await?
            .ok_or(Error::from(UseCaseError::UserNotFound))?;

        updated_user_guard
            .try_read(actor_ref.clone())
            .map(|user| user.into_inner())
            .map_err(Into::into)
    }

    pub async fn fetch_all_users(&self, actor: &ActiveUser) -> Result<Vec<ActiveUser>, Error> {
        let actor_ref = Actor::from(actor.clone());
        self.repository
            .fetch_all_users()
            .await?
            .into_iter()
            .map(|guard| {
                guard
                    .try_read(actor_ref.clone())
                    .map(|user| user.into_inner())
            })
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
                let user_ref = Actor::from(user.clone());
                self.repository
                    .upsert_user(
                        AuthorizationGuard::<_, Create>::from(user.to_owned())
                            .try_create(user_ref.clone())?,
                    )
                    .await?;
                // NOTE: リクエスト時点では token しかわからないので
                //  token で検索したユーザーが操作者であるとする
                self.repository
                    .find_by(user.id().into_inner())
                    .await?
                    .map(|guard| {
                        guard
                            .try_read(user_ref.clone())
                            .map(|user| user.into_inner())
                    })
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
            .link_discord_user(
                &discord_user,
                AuthorizationGuard::<_, Update>::from(user.clone())
                    .try_update(Actor::from(user))?,
            )
            .await
    }

    pub async fn unlink_discord_user(&self, user: ActiveUser) -> Result<(), Error> {
        self.repository
            .unlink_discord_user(
                AuthorizationGuard::<_, Update>::from(user.clone())
                    .try_update(Actor::from(user))?,
            )
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

        let allowed = guard.try_read(Actor::from(actor.clone()))?;
        let discord_user = self.repository.fetch_discord_user(&allowed).await?;

        let user = allowed.into_inner();

        Ok(UserProfile { user, discord_user })
    }
}
