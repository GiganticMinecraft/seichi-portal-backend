use domain::account::models::DiscordUserId;
use errors::{Error, infra::InfraError};
use serenity::all::UserId;

use crate::outgoing::connection::ConnectionPool;

impl ConnectionPool {
    pub async fn send_direct_message(
        &self,
        user_id: DiscordUserId,
        message: String,
    ) -> Result<(), Error> {
        let user_id = UserId::new(
            user_id
                .into_inner()
                .parse::<u64>()
                // NOTE: ここで失敗するのは Discord のユーザー id の仕様が変更されたときのみ
                .expect("Failed to parse DiscordUserId into u64"),
        );

        let http = &self.pool.http;

        let dm_channel = user_id
            .create_dm_channel(http)
            .await
            .map_err(Into::<InfraError>::into)?;

        dm_channel
            .say(&http, message)
            .await
            .map_err(Into::<InfraError>::into)?;

        Ok(())
    }
}
