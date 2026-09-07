use async_trait::async_trait;
use domain::{
    account::models::{
        AccountUser, DiscordAccountLink, DiscordUser, DiscordUserId, DiscordUserName, Role,
        UserGroup, UserGroupId, UserPagePosition, UserSessionLifetime,
    },
    pagination::{Page, PageRequest},
    repository::user_repository::UserRepository,
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Delete, Read, Update},
};
use errors::{Error, infra::InfraError};
use itertools::Itertools;
use reqwest::{
    StatusCode,
    header::{ACCEPT, CONTENT_TYPE, HeaderValue},
};
use reqwest_middleware::ClientWithMiddleware;
use serde::Deserialize;
use std::time::Duration;
use uuid::Uuid;

use crate::{
    database::components::{DatabaseComponents, UserDatabase},
    external::discord_api::DiscordAPI,
    outgoing::http::HTTP_CLIENT,
    repository::Repository,
};

const MINECRAFT_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
const MINECRAFT_PROFILE_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Deserialize)]
struct MinecraftProfileErrorResponse {
    path: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct MinecraftProfileResponse {
    id: Uuid,
    name: String,
}

fn is_missing_minecraft_profile_response(body: &str) -> bool {
    serde_json::from_str::<MinecraftProfileErrorResponse>(body)
        .map(|response| {
            response.path.as_deref() == Some("/minecraft/profile")
                && response.error.as_deref() == Some("NOT_FOUND")
        })
        .unwrap_or(false)
}

async fn fetch_minecraft_profile(
    client: &ClientWithMiddleware,
    url: &str,
    token: &str,
) -> Result<AccountUser, Error> {
    fetch_minecraft_profile_with_timeout(client, url, token, MINECRAFT_PROFILE_REQUEST_TIMEOUT)
        .await
}

async fn fetch_minecraft_profile_with_timeout(
    client: &ClientWithMiddleware,
    url: &str,
    token: &str,
    timeout: Duration,
) -> Result<AccountUser, Error> {
    let response = client
        .get(url)
        .bearer_auth(token)
        .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
        .header(ACCEPT, HeaderValue::from_static("application/json"))
        .timeout(timeout)
        .send()
        .await
        .map_err(|_| InfraError::MinecraftProfileRequestFailed)?;

    let status = response.status();
    if status == StatusCode::UNAUTHORIZED {
        return Err(InfraError::MinecraftTokenInvalid.into());
    }

    let body = response
        .text()
        .await
        .map_err(|_| InfraError::MinecraftProfileRequestFailed)?;

    if status == StatusCode::NOT_FOUND {
        return if is_missing_minecraft_profile_response(&body) {
            Err(InfraError::MinecraftProfileNotFound.into())
        } else {
            Err(InfraError::MinecraftProfileHttp {
                status: status.as_u16(),
            }
            .into())
        };
    }

    if status != StatusCode::OK {
        return Err(if status.is_success() {
            InfraError::MinecraftProfileInvalidResponse
        } else {
            InfraError::MinecraftProfileHttp {
                status: status.as_u16(),
            }
        }
        .into());
    }

    serde_json::from_str::<MinecraftProfileResponse>(&body)
        .map(|profile| AccountUser::new(profile.name, profile.id.into(), Role::default()))
        .map_err(|_| InfraError::MinecraftProfileInvalidResponse.into())
}

#[async_trait]
impl<Client: DatabaseComponents + 'static> UserRepository for Repository<Client> {
    async fn find_by(
        &self,
        uuid: Uuid,
    ) -> Result<Option<AuthorizationGuard<AccountUser, Read>>, Error> {
        Ok(self.client.user().find_by(uuid).await?.map(Into::into))
    }

    async fn find_by_ids(
        &self,
        uuids: Vec<Uuid>,
    ) -> Result<Vec<AuthorizationGuard<AccountUser, Read>>, Error> {
        Ok(self
            .client
            .user()
            .find_by_ids(uuids)
            .await?
            .into_iter()
            .map(Into::into)
            .collect_vec())
    }

    async fn upsert_user(&self, user: Allowed<AccountUser, Create>) -> Result<(), Error> {
        self.client
            .user()
            .upsert_user(user.value())
            .await
            .map_err(Into::into)
    }

    async fn patch_user_role(&self, user: Allowed<AccountUser, Update>) -> Result<(), Error> {
        self.client
            .user()
            .patch_user_role(
                user.value().id().into_inner(),
                user.value().role().to_owned(),
            )
            .await
            .map_err(Into::into)
    }

    async fn create_user_group(&self, group: Allowed<UserGroup, Create>) -> Result<(), Error> {
        self.client
            .user()
            .create_user_group(group.value())
            .await
            .map_err(Into::into)
    }

    async fn update_user_group(&self, group: Allowed<UserGroup, Update>) -> Result<(), Error> {
        self.client
            .user()
            .update_user_group(group.value())
            .await
            .map_err(Into::into)
    }

    async fn delete_user_group(&self, group: Allowed<UserGroup, Delete>) -> Result<(), Error> {
        self.client
            .user()
            .delete_user_group(*group.id())
            .await
            .map_err(Into::into)
    }

    async fn find_user_group(
        &self,
        group_id: UserGroupId,
    ) -> Result<Option<AuthorizationGuard<UserGroup, Read>>, Error> {
        Ok(self
            .client
            .user()
            .find_user_group(group_id)
            .await?
            .map(Into::into))
    }

    async fn fetch_user_groups(&self) -> Result<Vec<AuthorizationGuard<UserGroup, Read>>, Error> {
        Ok(self
            .client
            .user()
            .fetch_user_groups()
            .await?
            .into_iter()
            .map(Into::into)
            .collect_vec())
    }

    async fn fetch_users_by_group(
        &self,
        group: Allowed<UserGroup, Read>,
    ) -> Result<Vec<AuthorizationGuard<AccountUser, Read>>, Error> {
        Ok(self
            .client
            .user()
            .fetch_users_by_group(*group.id())
            .await?
            .into_iter()
            .map(Into::into)
            .collect_vec())
    }

    async fn add_user_to_group(
        &self,
        group: Allowed<UserGroup, Update>,
        user: Allowed<AccountUser, Update>,
    ) -> Result<(), Error> {
        self.client
            .user()
            .add_user_to_group(*group.id(), user.id().into_inner())
            .await
            .map_err(Into::into)
    }

    async fn remove_user_from_group(
        &self,
        group: Allowed<UserGroup, Update>,
        user: Allowed<AccountUser, Update>,
    ) -> Result<(), Error> {
        self.client
            .user()
            .remove_user_from_group(*group.id(), user.id().into_inner())
            .await
            .map_err(Into::into)
    }

    async fn fetch_user_by_xbox_token(&self, token: String) -> Result<AccountUser, Error> {
        fetch_minecraft_profile(&HTTP_CLIENT, MINECRAFT_PROFILE_URL, &token).await
    }

    async fn fetch_all_users(&self) -> Result<Vec<AuthorizationGuard<AccountUser, Read>>, Error> {
        Ok(self
            .client
            .user()
            .fetch_all_users()
            .await?
            .into_iter()
            .map(Into::into)
            .collect_vec())
    }

    async fn fetch_users_page(
        &self,
        request: PageRequest<UserPagePosition>,
    ) -> Result<Page<AuthorizationGuard<AccountUser, Read>, UserPagePosition>, Error> {
        let page = self.client.user().fetch_users_page(request).await?;
        let (users, next) = page.into_parts();

        Ok(Page::new(
            users.into_iter().map(Into::into).collect_vec(),
            next,
        ))
    }

    async fn start_user_session(
        &self,
        xbox_token: String,
        user: &AccountUser,
        lifetime: UserSessionLifetime,
    ) -> Result<String, Error> {
        self.client
            .user()
            .start_user_session(xbox_token, user, lifetime)
            .await
            .map_err(Into::into)
    }

    async fn fetch_user_by_session_id(
        &self,
        session_id: String,
    ) -> Result<Option<AccountUser>, Error> {
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
        user: &Allowed<AccountUser, Read>,
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

#[cfg(test)]
mod tests {
    use axum::{
        Router,
        http::{StatusCode as MockStatusCode, header::CONTENT_TYPE},
        routing::get,
    };
    use reqwest_middleware::ClientBuilder;
    use tokio::{
        net::TcpListener,
        task::JoinHandle,
        time::{Duration, sleep},
    };

    use super::*;

    const VALID_PROFILE: &str = r#"{
        "id": "478911be335646c1936efb14b71bf282",
        "name": "test_user",
        "skins": [],
        "capes": []
    }"#;

    async fn spawn_profile_server(
        status: MockStatusCode,
        body: &'static str,
    ) -> (String, JoinHandle<()>) {
        let app = Router::new().route(
            "/minecraft/profile",
            get(move || async move { (status, [(CONTENT_TYPE, "application/json")], body) }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        (format!("http://{address}/minecraft/profile"), server)
    }

    async fn spawn_delayed_profile_server(delay: Duration) -> (String, JoinHandle<()>) {
        let app = Router::new().route(
            "/minecraft/profile",
            get(move || async move {
                sleep(delay).await;
                (
                    MockStatusCode::OK,
                    [(CONTENT_TYPE, "application/json")],
                    VALID_PROFILE,
                )
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });

        (format!("http://{address}/minecraft/profile"), server)
    }

    fn client() -> ClientWithMiddleware {
        ClientBuilder::new(reqwest::Client::new()).build()
    }

    #[tokio::test]
    async fn valid_profile_response_preserves_normal_login_data() {
        let (url, server) = spawn_profile_server(MockStatusCode::OK, VALID_PROFILE).await;

        let result = fetch_minecraft_profile(&client(), &url, "test-token")
            .await
            .unwrap();

        server.abort();
        assert_eq!(result.name(), "test_user");
        assert_eq!(
            result.id().to_string(),
            "478911be-3356-46c1-936e-fb14b71bf282"
        );
        assert_eq!(result.role(), &Role::StandardUser);
    }

    #[tokio::test]
    async fn profile_not_found_requires_the_documented_error_shape() {
        let body = r#"{
            "path": "/minecraft/profile",
            "error": "NOT_FOUND",
            "errorMessage": "Not Found"
        }"#;
        let (url, server) = spawn_profile_server(MockStatusCode::NOT_FOUND, body).await;

        let result = fetch_minecraft_profile(&client(), &url, "test-token").await;

        server.abort();
        assert_eq!(result, Err(InfraError::MinecraftProfileNotFound.into()));
    }

    #[tokio::test]
    async fn an_unrecognized_not_found_response_remains_an_upstream_error() {
        let body = r#"{
            "path": "/minecraft/profile",
            "error": "SOME_OTHER_ERROR"
        }"#;
        let (url, server) = spawn_profile_server(MockStatusCode::NOT_FOUND, body).await;

        let result = fetch_minecraft_profile(&client(), &url, "test-token").await;

        server.abort();
        assert_eq!(
            result,
            Err(InfraError::MinecraftProfileHttp { status: 404 }.into())
        );
    }

    #[tokio::test]
    async fn unauthorized_profile_response_is_classified_as_an_invalid_token() {
        let (url, server) = spawn_profile_server(
            MockStatusCode::UNAUTHORIZED,
            r#"{"path":"/minecraft/profile"}"#,
        )
        .await;

        let result = fetch_minecraft_profile(&client(), &url, "test-token").await;

        server.abort();
        assert_eq!(result, Err(InfraError::MinecraftTokenInvalid.into()));
    }

    #[tokio::test]
    async fn upstream_server_errors_are_not_classified_as_account_errors() {
        let (url, server) = spawn_profile_server(
            MockStatusCode::INTERNAL_SERVER_ERROR,
            r#"{"message":"temporary failure"}"#,
        )
        .await;

        let result = fetch_minecraft_profile(&client(), &url, "test-token").await;

        server.abort();
        assert_eq!(
            result,
            Err(InfraError::MinecraftProfileHttp { status: 500 }.into())
        );
    }

    #[tokio::test]
    async fn malformed_success_response_is_not_classified_as_account_error() {
        let (url, server) = spawn_profile_server(
            MockStatusCode::OK,
            r#"{"id":"not-a-uuid","name":"test_user"}"#,
        )
        .await;

        let result = fetch_minecraft_profile(&client(), &url, "test-token").await;

        server.abort();
        assert_eq!(
            result,
            Err(InfraError::MinecraftProfileInvalidResponse.into())
        );
    }

    #[tokio::test]
    async fn profile_request_timeout_is_classified_as_communication_failure() {
        let (url, server) = spawn_delayed_profile_server(Duration::from_millis(100)).await;

        let test_client = client();
        let result = fetch_minecraft_profile_with_timeout(
            &test_client,
            &url,
            "test-token",
            Duration::from_millis(10),
        )
        .await;

        server.abort();
        assert_eq!(
            result,
            Err(InfraError::MinecraftProfileRequestFailed.into())
        );
    }
}
