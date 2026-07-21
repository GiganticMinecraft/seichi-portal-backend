mod config;
pub mod connection;
pub mod discord_sender_impl;
pub mod discord_webhook_sender;

pub use config::{DiscordGlobalWebhookUrl, load_discord_global_webhook_url};
