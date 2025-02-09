use serde::Deserialize;

// ref: https://discord.com/developers/docs/resources/user#user-object
// NOTE: 2025/02/08 時点で Discord から取得できるスキーマを示しているが、
//  id と username だけ取得できれば実装時点での機能要求的に十分である。
#[derive(Deserialize, Debug)]
pub struct DiscordUserSchema {
    pub id: String,
    pub username: String,
}
