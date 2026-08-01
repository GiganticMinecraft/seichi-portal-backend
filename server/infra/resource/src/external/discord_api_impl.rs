use async_trait::async_trait;
use errors::infra::InfraError;

use crate::{
    database::connection::ConnectionPool,
    external::{discord_api::DiscordAPI, discord_api_schema::DiscordUserSchema},
    outgoing::http::HTTP_CLIENT,
};

#[async_trait]
impl DiscordAPI for ConnectionPool {
    async fn fetch_user(&self, token: String) -> Result<DiscordUserSchema, InfraError> {
        let response = HTTP_CLIENT
            .get("https://discord.com/api/users/@me")
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await?;

        serde_json::from_str(response.text().await?.as_str()).map_err(Into::into)
    }
}
