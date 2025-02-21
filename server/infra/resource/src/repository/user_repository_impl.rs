use async_trait::async_trait;
use common::config::ENV;
use domain::{
    repository::user_repository::UserRepository,
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard_with_context::{Create, Read, Update},
    },
    user::models::{DiscordUser, DiscordUserId, DiscordUserName, Role::Administrator, User},
};
use errors::{Error, infra::InfraError::Reqwest};
use itertools::Itertools;
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderValue};
use uuid::{Uuid, uuid};

use crate::{
    database::components::{DatabaseComponents, UserDatabase},
    external::discord_api::DiscordAPI,
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> UserRepository for Repository<Client> {
    async fn find_by(&self, uuid: Uuid) -> Result<Option<AuthorizationGuard<User, Read>>, Error> {
        Ok(self.client.user().find_by(uuid).await?.map(Into::into))
    }

    async fn upsert_user(
        &self,
        actor: &User,
        user: AuthorizationGuard<User, Create>,
    ) -> Result<(), Error> {
        user.try_create(actor, |user| self.client.user().upsert_user(user))?
            .await
            .map_err(Into::into)
    }

    async fn patch_user_role(
        &self,
        actor: &User,
        user: AuthorizationGuard<User, Update>,
    ) -> Result<(), Error> {
        user.try_update(actor, |user| {
            self.client
                .user()
                .patch_user_role(user.id, user.role.to_owned())
        })?
        .await
        .map_err(Into::into)
    }

    async fn fetch_user_by_xbox_token(&self, token: String) -> Result<Option<User>, Error> {
        Ok(if ENV.name == "local" && token == "debug_user" {
            Some(User {
                name: "test_user".to_string(),
                id: uuid!("478911be-3356-46c1-936e-fb14b71bf282"),
                role: Administrator,
            })
        } else {
            let client = reqwest::Client::new();

            let response = client
                .get("https://api.minecraftservices.com/minecraft/profile")
                .bearer_auth(token.to_owned())
                .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
                .header(ACCEPT, HeaderValue::from_static("application/json"))
                .send()
                .await
                .map_err(|err| Reqwest {
                    cause: err.to_string(),
                })?;

            serde_json::from_str::<User>(
                response
                    .text()
                    .await
                    .map_err(|err| Reqwest {
                        cause: err.to_string(),
                    })?
                    .as_str(),
            )
            .ok()
        })
    }

    async fn fetch_all_users(&self) -> Result<Vec<AuthorizationGuard<User, Read>>, Error> {
        Ok(self
            .client
            .user()
            .fetch_all_users()
            .await?
            .into_iter()
            .map(Into::into)
            .collect_vec())
    }

    async fn start_user_session(
        &self,
        xbox_token: String,
        user: &User,
        expires: i32,
    ) -> Result<String, Error> {
        self.client
            .user()
            .start_user_session(xbox_token, user, expires)
            .await
            .map_err(Into::into)
    }

    async fn fetch_user_by_session_id(&self, session_id: String) -> Result<Option<User>, Error> {
        Ok(self
            .client
            .user()
            .fetch_user_by_session_id(session_id)
            .await?)
    }

    async fn end_user_session(&self, session_id: String) -> Result<(), Error> {
        self.client
            .user()
            .end_user_session(session_id)
            .await
            .map_err(Into::into)
    }

    async fn link_discord_user(
        &self,
        actor: &User,
        discord_user: &DiscordUser,
        user: AuthorizationGuard<User, Update>,
    ) -> Result<(), Error> {
        user.try_update(actor, |user| {
            self.client.user().link_discord_user(discord_user, user)
        })?
        .await
        .map_err(Into::into)
    }

    async fn unlink_discord_user(
        &self,
        actor: &User,
        user: AuthorizationGuard<User, Update>,
    ) -> Result<(), Error> {
        user.try_update(actor, |user| self.client.user().unlink_discord_user(user))?
            .await
            .map_err(Into::into)
    }

    async fn fetch_discord_user(
        &self,
        actor: &User,
        user: &AuthorizationGuard<User, Read>,
    ) -> Result<Option<DiscordUser>, Error> {
        let user = user.try_read(actor)?;

        Ok(self
            .client
            .user()
            .fetch_discord_user(user)
            .await?
            .map(Into::into))
    }

    async fn fetch_discord_user_by_token(
        &self,
        token: String,
    ) -> Result<Option<DiscordUser>, Error> {
        Ok(self
            .client
            .discord_api()
            .fetch_user(token)
            .await
            .ok()
            .map(|schema| {
                DiscordUser::new(
                    DiscordUserId::new(schema.id),
                    DiscordUserName::new(schema.username),
                )
            }))
    }
}
