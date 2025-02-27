use std::str::FromStr;

use async_trait::async_trait;
use domain::{notification::models::NotificationSettings, user::models::Role};
use errors::infra::InfraError;
use uuid::Uuid;

use crate::{
    database::{
        components::NotificationDatabase,
        connection::{ConnectionPool, execute_and_values, query_one_and_values},
    },
    dto::{NotificationSettingsDto, UserDto},
};

#[async_trait]
impl NotificationDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn upsert_notification_settings(
        &self,
        notification_settings: &NotificationSettings,
    ) -> Result<(), InfraError> {
        let params = [
            notification_settings.recipient().id.to_string().into(),
            notification_settings
                .is_send_message_notification()
                .to_owned()
                .into(),
        ];

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    r"INSERT INTO discord_notification_settings (discord_id, is_send_message_notification)
                    VALUES ((SELECT discord_id FROM discord_linked_users WHERE user_id = ?), ?)
                    ON DUPLICATE KEY UPDATE
                    is_send_message_notification = VALUES(is_send_message_notification)
                    ",
                    params,
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn fetch_notification_settings(
        &self,
        recipient_id: Uuid,
    ) -> Result<Option<NotificationSettingsDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let rs = query_one_and_values(
                    r"SELECT is_send_message_notification, name, role
                    FROM discord_notification_settings
                    INNER JOIN discord_linked_users ON discord_notification_settings.discord_id = discord_linked_users.discord_id
                    INNER JOIN users ON discord_linked_users.user_id = users.id
                    WHERE user_id = ?",
                    [recipient_id.to_string().into()],
                    txn,
                ).await?;

                rs.map(|rs| {
                    Ok::<_, InfraError>(NotificationSettingsDto {
                        recipient: UserDto {
                            name: rs.try_get("", "name")?,
                            id: recipient_id.to_string(),
                            role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                        },
                        is_send_message_notification: rs.try_get("", "is_send_message_notification")?,
                    })
                }).transpose()
            })
        }).await.map_err(Into::into)
    }
}
