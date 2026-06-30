use domain::{
    account::models::{
        AccountUser, DiscordAccountLink, Role, UserGroup, UserGroupId, UserGroupName,
        UserPagePosition,
    },
    auth::Actor,
    pagination::{Page, PageRequest},
    repository::user_repository::UserRepository,
    types::authorization_guard::{AuthorizationGuard, Create, Delete, Read, Update},
};
use errors::{Error, usecase::UseCaseError};
use uuid::Uuid;

use crate::models::UserProfile;

pub struct UserUseCase<'a, UserRepo: UserRepository> {
    pub repository: &'a UserRepo,
}

impl<R: UserRepository> UserUseCase<'_, R> {
    pub async fn find_by(&self, actor: &AccountUser, uuid: Uuid) -> Result<AccountUser, Error> {
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
        actor: &AccountUser,
        upsert_target: AccountUser,
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
        actor: &AccountUser,
        uuid: Uuid,
        role: Role,
    ) -> Result<AccountUser, Error> {
        let actor_ref = Actor::from(actor.clone());
        let current_user_guard = self
            .repository
            .find_by(uuid)
            .await?
            .ok_or(Error::from(UseCaseError::UserNotFound))?;

        let current_user = current_user_guard.try_read(actor_ref.clone())?.into_inner();
        let new_role_user = AccountUser::with_groups(
            current_user.name().to_owned(),
            *current_user.id(),
            role,
            current_user.groups().to_vec(),
        );

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

    pub async fn create_user_group(
        &self,
        actor: &AccountUser,
        name: UserGroupName,
    ) -> Result<UserGroup, Error> {
        let actor_ref = Actor::from(actor.clone());
        let group = UserGroup::new(name);
        let group_id = *group.id();

        self.repository
            .create_user_group(
                AuthorizationGuard::<_, Create>::from(group).try_create(actor_ref.clone())?,
            )
            .await?;

        self.repository
            .find_user_group(group_id)
            .await?
            .ok_or(Error::from(UseCaseError::UserGroupNotFound))?
            .try_read(actor_ref)
            .map(|group| group.into_inner())
            .map_err(Into::into)
    }

    pub async fn update_user_group(
        &self,
        actor: &AccountUser,
        group_id: UserGroupId,
        name: UserGroupName,
    ) -> Result<UserGroup, Error> {
        let actor_ref = Actor::from(actor.clone());
        let current_group = self
            .repository
            .find_user_group(group_id)
            .await?
            .ok_or(Error::from(UseCaseError::UserGroupNotFound))?
            .try_read(actor_ref.clone())?
            .into_inner();
        let updated_group = unsafe { UserGroup::from_raw_parts(*current_group.id(), name) };

        self.repository
            .update_user_group(
                AuthorizationGuard::<_, Update>::from(updated_group)
                    .try_update(actor_ref.clone())?,
            )
            .await?;

        self.repository
            .find_user_group(group_id)
            .await?
            .ok_or(Error::from(UseCaseError::UserGroupNotFound))?
            .try_read(actor_ref)
            .map(|group| group.into_inner())
            .map_err(Into::into)
    }

    pub async fn delete_user_group(
        &self,
        actor: &AccountUser,
        group_id: UserGroupId,
    ) -> Result<(), Error> {
        let actor_ref = Actor::from(actor.clone());
        let group = self
            .repository
            .find_user_group(group_id)
            .await?
            .ok_or(Error::from(UseCaseError::UserGroupNotFound))?;

        self.repository
            .delete_user_group(group.into_delete().try_delete(actor_ref)?)
            .await
    }

    pub async fn fetch_user_groups(&self, actor: &AccountUser) -> Result<Vec<UserGroup>, Error> {
        let actor_ref = Actor::from(actor.clone());
        self.repository
            .fetch_user_groups()
            .await?
            .into_iter()
            .map(|guard| {
                guard
                    .try_read(actor_ref.clone())
                    .map(|group| group.into_inner())
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub async fn add_user_to_group(
        &self,
        actor: &AccountUser,
        group_id: UserGroupId,
        user_id: Uuid,
    ) -> Result<AccountUser, Error> {
        self.update_user_group_membership(actor, group_id, user_id, true)
            .await
    }

    pub async fn remove_user_from_group(
        &self,
        actor: &AccountUser,
        group_id: UserGroupId,
        user_id: Uuid,
    ) -> Result<AccountUser, Error> {
        self.update_user_group_membership(actor, group_id, user_id, false)
            .await
    }

    async fn update_user_group_membership(
        &self,
        actor: &AccountUser,
        group_id: UserGroupId,
        user_id: Uuid,
        should_belong: bool,
    ) -> Result<AccountUser, Error> {
        let actor_ref = Actor::from(actor.clone());
        let group = self
            .repository
            .find_user_group(group_id)
            .await?
            .ok_or(Error::from(UseCaseError::UserGroupNotFound))?;
        let user = self
            .repository
            .find_by(user_id)
            .await?
            .ok_or(Error::from(UseCaseError::UserNotFound))?;

        let allowed_group = group.into_update().try_update(actor_ref.clone())?;
        let allowed_user = user.into_update().try_update(actor_ref.clone())?;

        if should_belong {
            self.repository
                .add_user_to_group(allowed_group, allowed_user)
                .await?;
        } else {
            self.repository
                .remove_user_from_group(allowed_group, allowed_user)
                .await?;
        }

        self.find_by(actor, user_id).await
    }

    pub async fn fetch_all_users(&self, actor: &AccountUser) -> Result<Vec<AccountUser>, Error> {
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

    pub async fn fetch_users_page(
        &self,
        actor: &AccountUser,
        request: PageRequest<UserPagePosition>,
    ) -> Result<Page<AccountUser, UserPagePosition>, Error> {
        let actor_ref = Actor::from(actor.clone());
        let page = self.repository.fetch_users_page(request).await?;
        let (users, next) = page.into_parts();
        let users = users
            .into_iter()
            .map(|guard| {
                guard
                    .try_read(actor_ref.clone())
                    .map(|user| user.into_inner())
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Page::new(users, next))
    }

    pub async fn fetch_user_by_xbox_token(
        &self,
        token: String,
    ) -> Result<Option<AccountUser>, Error> {
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
        user: &AccountUser,
        expires: u32,
    ) -> Result<String, Error> {
        self.repository
            .start_user_session(xbox_token, user, expires)
            .await
    }

    pub async fn fetch_user_by_session_id(
        &self,
        session_id: String,
    ) -> Result<Option<AccountUser>, Error> {
        self.repository.fetch_user_by_session_id(session_id).await
    }

    pub async fn end_user_session(&self, session_id: String) -> Result<(), Error> {
        self.repository.end_user_session(session_id).await
    }

    pub async fn link_discord_user(
        &self,
        discord_oauth_token: String,
        user: AccountUser,
    ) -> Result<(), Error> {
        let discord_user = self
            .repository
            .fetch_discord_user_by_token(discord_oauth_token)
            .await?
            .ok_or(Error::from(UseCaseError::DiscordLinkFailed))?;

        let link = DiscordAccountLink::new(*user.id(), discord_user);

        self.repository
            .link_discord_user(
                AuthorizationGuard::<_, Update>::from(link).try_update(Actor::from(user))?,
            )
            .await
    }

    pub async fn unlink_discord_user(&self, user: AccountUser) -> Result<(), Error> {
        let allowed_user = AuthorizationGuard::<_, Read>::from(user.clone())
            .try_read(Actor::from(user.clone()))?;
        let discord_user = self.repository.fetch_discord_user(&allowed_user).await?;

        let Some(discord_user) = discord_user else {
            return Ok(());
        };

        let link = DiscordAccountLink::new(*user.id(), discord_user);

        self.repository
            .unlink_discord_user(
                AuthorizationGuard::<_, Delete>::from(link).try_delete(Actor::from(user))?,
            )
            .await
    }

    pub async fn fetch_user_information(
        &self,
        actor: &AccountUser,
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
