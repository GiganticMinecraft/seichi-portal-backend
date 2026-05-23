use std::str::FromStr;

use async_trait::async_trait;
use domain::{notification::models::NotificationPreference, user::models::Role};
use errors::infra::InfraError;
use uuid::Uuid;

use crate::{
    database::{components::NotificationDatabase, connection::ConnectionPool},
    records::{NotificationSettingsRecord, UserRecord},
};

#[async_trait]
impl NotificationDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn upsert_notification_settings(
        &self,
        notification_settings: &NotificationPreference,
    ) -> Result<(), InfraError> {
        let recipient_id = notification_settings.recipient_id().to_string();
        let is_send_message_notification = *notification_settings.is_send_message_notification();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    r"INSERT INTO discord_notification_settings (discord_id, is_send_message_notification)
                    VALUES ((SELECT discord_id FROM discord_linked_users WHERE user_id = ?), ?)
                    ON DUPLICATE KEY UPDATE
                    is_send_message_notification = VALUES(is_send_message_notification)
                    ",
                    recipient_id,
                    is_send_message_notification,
                )
                .execute(&mut **txn)
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn fetch_notification_settings(
        &self,
        recipient_id: Uuid,
    ) -> Result<Option<NotificationSettingsRecord>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let rs = sqlx::query!(
                    r"SELECT is_send_message_notification, name, role
                    FROM discord_notification_settings
                    INNER JOIN discord_linked_users ON discord_notification_settings.discord_id = discord_linked_users.discord_id
                    INNER JOIN users ON discord_linked_users.user_id = users.id
                    WHERE user_id = ?",
                    recipient_id.to_string(),
                )
                .fetch_optional(&mut **txn)
                .await?;

                rs.map(|row| {
                    Ok::<_, InfraError>(NotificationSettingsRecord {
                        recipient: UserRecord {
                            name: row.name,
                            id: recipient_id.to_string(),
                            role: Role::from_str(&row.role)?,
                        },
                        is_send_message_notification: row.is_send_message_notification != 0,
                    })
                })
                .transpose()
            })
        })
        .await
    }
}
