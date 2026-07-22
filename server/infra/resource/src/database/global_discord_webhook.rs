use errors::infra::InfraError;

use crate::database::connection::ConnectionPool;

impl ConnectionPool {
    pub(crate) async fn fetch_global_discord_webhook_url(
        &self,
    ) -> Result<Option<String>, InfraError> {
        let row = sqlx::query!(
            r"SELECT url FROM global_discord_webhook_settings WHERE singleton_key = 1"
        )
        .fetch_one(&self.rdb_pool)
        .await?;

        Ok(row.url)
    }

    pub(crate) async fn update_global_discord_webhook_url(
        &self,
        url: Option<&str>,
    ) -> Result<(), InfraError> {
        sqlx::query!(
            r"UPDATE global_discord_webhook_settings SET url = ? WHERE singleton_key = 1",
            url,
        )
        .execute(&self.rdb_pool)
        .await?;

        Ok(())
    }
}
