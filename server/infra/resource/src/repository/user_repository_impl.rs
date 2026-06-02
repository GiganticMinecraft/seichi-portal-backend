use async_trait::async_trait;
use domain::{
    repository::user_repository::UserRepository,
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Delete, Read, Update},
    user::models::{ActiveUser, DiscordAccountLink, DiscordUser, DiscordUserId, DiscordUserName},
};
use errors::{Error, infra::InfraError::Reqwest};
use itertools::Itertools;
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderValue};
use uuid::Uuid;

use crate::{
    database::components::{DatabaseComponents, UserDatabase},
    external::discord_api::DiscordAPI,
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> UserRepository for Repository<Client> {
    async fn find_by(
        &self,
        uuid: Uuid,
    ) -> Result<Option<AuthorizationGuard<ActiveUser, Read>>, Error> {
        Ok(self.client.user().find_by(uuid).await?.map(Into::into))
    }

    async fn find_by_ids(
        &self,
        uuids: Vec<Uuid>,
    ) -> Result<Vec<AuthorizationGuard<ActiveUser, Read>>, Error> {
        Ok(self
            .client
            .user()
            .find_by_ids(uuids)
            .await?
            .into_iter()
            .map(Into::into)
            .collect_vec())
    }

    async fn upsert_user(&self, user: Allowed<ActiveUser, Create>) -> Result<(), Error> {
        self.client
            .user()
            .upsert_user(user.value())
            .await
            .map_err(Into::into)
    }

    async fn patch_user_role(&self, user: Allowed<ActiveUser, Update>) -> Result<(), Error> {
        self.client
            .user()
            .patch_user_role(
                user.value().id().into_inner(),
                user.value().role().to_owned(),
            )
            .await
            .map_err(Into::into)
    }

    async fn fetch_user_by_xbox_token(&self, token: String) -> Result<Option<ActiveUser>, Error> {
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

        Ok(serde_json::from_str::<ActiveUser>(
            response
                .text()
                .await
                .map_err(|err| Reqwest {
                    cause: err.to_string(),
                })?
                .as_str(),
        )
        .ok())
    }

    async fn fetch_all_users(&self) -> Result<Vec<AuthorizationGuard<ActiveUser, Read>>, Error> {
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
        user: &ActiveUser,
        expires: u32,
    ) -> Result<String, Error> {
        self.client
            .user()
            .start_user_session(xbox_token, user, expires)
            .await
            .map_err(Into::into)
    }

    async fn fetch_user_by_session_id(
        &self,
        session_id: String,
    ) -> Result<Option<ActiveUser>, Error> {
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
        link: Allowed<DiscordAccountLink, Update>,
    ) -> Result<(), Error> {
        self.client
            .user()
            .link_discord_user(link.value())
            .await
            .map_err(Into::into)
    }

    async fn unlink_discord_user(
        &self,
        link: Allowed<DiscordAccountLink, Delete>,
    ) -> Result<(), Error> {
        self.client
            .user()
            .unlink_discord_user(link.value())
            .await
            .map_err(Into::into)
    }

    async fn fetch_discord_user(
        &self,
        user: &Allowed<ActiveUser, Read>,
    ) -> Result<Option<DiscordUser>, Error> {
        Ok(self
            .client
            .user()
            .fetch_discord_user(user.value())
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

    async fn size(&self) -> Result<u32, Error> {
        self.client.user().fetch_size().await.map_err(Into::into)
    }
}
