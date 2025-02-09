use std::sync::LazyLock;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Discord {
    pub bot_token: String,
}

pub static DISCORD_BOT: LazyLock<Discord> =
    LazyLock::new(|| envy::prefixed("DISCORD_").from_env::<Discord>().unwrap());
