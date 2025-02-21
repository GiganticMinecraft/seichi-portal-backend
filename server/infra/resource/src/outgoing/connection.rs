use serenity::{Client, all::GatewayIntents, client::ClientBuilder};

use crate::outgoing::config::{DISCORD_BOT, Discord};

pub struct ConnectionPool {
    pub pool: Client,
}

impl ConnectionPool {
    pub async fn new() -> Self {
        let Discord { bot_token } = &*DISCORD_BOT;

        let intents = GatewayIntents::DIRECT_MESSAGES;

        let client = ClientBuilder::new(bot_token, intents)
            .await
            .expect("Discord client creation failed.");

        Self { pool: client }
    }
}
