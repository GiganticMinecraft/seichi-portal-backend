use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct DiscordOAuthToken {
    pub token: String,
}
