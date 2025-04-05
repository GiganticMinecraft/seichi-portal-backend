use domain::notification::discord_dm_notificator::DiscordDMNotificator;

pub struct DiscordDMNotificatorImpl {}

impl Default for DiscordDMNotificatorImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscordDMNotificatorImpl {
    pub const fn new() -> Self {
        Self {}
    }
}

impl DiscordDMNotificator for DiscordDMNotificatorImpl {}
