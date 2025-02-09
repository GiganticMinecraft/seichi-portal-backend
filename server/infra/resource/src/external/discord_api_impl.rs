use async_trait::async_trait;
use errors::infra::InfraError;

use crate::{
    database::connection::ConnectionPool,
    external::{discord_api::DiscordAPI, discord_api_schema::DiscordUserSchema},
};

#[async_trait]
impl DiscordAPI for ConnectionPool {
    async fn fetch_user(&self, token: String) -> Result<DiscordUserSchema, InfraError> {
        let client = reqwest::Client::new();
        let response = client
            .get("https://discord.com/api/users/@me")
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|err| InfraError::Reqwest {
                cause: err.to_string(),
            })?;

        serde_json::from_str(
            response
                .text()
                .await
                .map_err(|err| InfraError::Reqwest {
                    cause: err.to_string(),
                })?
                .as_str(),
        )
        .map_err(Into::into)
    }
}
