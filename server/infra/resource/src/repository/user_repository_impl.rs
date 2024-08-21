use async_trait::async_trait;
use common::config::ENV;
use domain::{
    repository::user_repository::UserRepository,
    user::models::{Role, Role::Administrator, User},
};
use errors::{infra::InfraError::Reqwest, Error};
use reqwest::header::{HeaderValue, ACCEPT, CONTENT_TYPE};
use uuid::{uuid, Uuid};

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
        self.client
            .user()
            .fetch_user_by_session_id(session_id)
            .await
            .map_err(Into::into)
    }

    async fn end_user_session(&self, session_id: String) -> Result<(), Error> {
        self.client
            .user()
            .end_user_session(session_id)
            .await
            .map_err(Into::into)
    }
}
