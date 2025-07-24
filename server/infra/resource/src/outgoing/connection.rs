use crate::outgoing::config::{DISCORD_BOT, Discord};
use serenity::{Client, all::GatewayIntents, client::ClientBuilder};

pub struct ConnectionPool {
    pub pool: Option<Client>,
}

impl ConnectionPool {
    pub async fn new() -> Self {
        let Discord { bot_token } = &*DISCORD_BOT;

        let intents = GatewayIntents::DIRECT_MESSAGES;

        match bot_token.to_owned() {
            Some(token) => Self {
                pool: Some(
                    ClientBuilder::new(token, intents)
                        .await
                        .expect("Discord client creation failed."),
                ),
            },
            None => Self { pool: None },
        }
    }
}
