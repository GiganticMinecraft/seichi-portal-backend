use std::str::FromStr;

use async_trait::async_trait;
use domain::{
    account::models::{Role, UserId},
    notification::models::{
        Notification, NotificationId, NotificationPagePosition, NotificationPreference,
    },
    pagination::{Page, PageRequest},
};
use errors::infra::InfraError;
use uuid::Uuid;

use crate::{
    database::{components::NotificationDatabase, connection::ConnectionPool},
    records::{NotificationRecord, NotificationSettingsRecord, UserRecord},
};

#[async_trait]
impl NotificationDatabase for ConnectionPool {
    #[tracing::instrument(skip_all, fields(notification_id = %notification.id()))]
    async fn create_notification(&self, notification: &Notification) -> Result<(), InfraError> {
        let id = notification.id().to_string();
        let recipient_id = notification.recipient_id().to_string();
        let notification_type = notification.notification_type().to_string();
        let related_answer_id = notification.related_answer_id().to_string();
        let title = notification.content().title().to_owned();
        let body = notification.content().body().to_owned();
        let url = notification.content().url().to_owned();
        let created_at = *notification.created_at();
        let read_at = *notification.read_at();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    r"INSERT INTO notifications
                    (id, recipient_id, notification_type, related_answer_id, title, body, url, created_at, read_at)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
                    id,
                    recipient_id,
                    notification_type,
                    related_answer_id,
                    title,
                    body,
                    url,
                    created_at,
                    read_at,
                )
                .execute(&mut **txn)
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument(skip_all, fields(notification_id = %id))]
    async fn fetch_notification(
        &self,
        id: NotificationId,
    ) -> Result<Option<NotificationRecord>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                sqlx::query_as!(
                    NotificationRecord,
                    r"SELECT id, recipient_id, notification_type, related_answer_id,
                        title, body, url, created_at AS `created_at!: chrono::DateTime<chrono::Utc>`,
                        read_at
                    FROM notifications
                    WHERE id = ?",
                    id.to_string(),
                )
                .fetch_optional(&mut **txn)
                .await
                .map_err(Into::<InfraError>::into)
            })
        })
        .await
    }

    #[tracing::instrument(skip_all, fields(recipient_id = %recipient_id))]
    async fn fetch_notifications(
        &self,
        recipient_id: UserId,
        request: PageRequest<NotificationPagePosition>,
    ) -> Result<Page<NotificationRecord, NotificationPagePosition>, InfraError> {
        let recipient_id = recipient_id.to_string();
        let after = request
            .after_position()
            .map(|position| position.id().to_string());
        let limit = request.limit();
        let overfetch = i64::from(limit.overfetch_value());

        let rows = self
            .read_only_transaction(|txn| {
                Box::pin(async move {
                    let rows = match after {
                        Some(after) => {
                            sqlx::query_as!(
                                NotificationRecord,
                                r"SELECT id, recipient_id, notification_type, related_answer_id,
                                    title, body, url, created_at AS `created_at!: chrono::DateTime<chrono::Utc>`,
                                    read_at
                                FROM notifications
                                WHERE recipient_id = ? AND id < ?
                                ORDER BY id DESC LIMIT ?",
                                recipient_id,
                                after,
                                overfetch,
                            )
                            .fetch_all(&mut **txn)
                            .await?
                        }
                        None => {
                            sqlx::query_as!(
                                NotificationRecord,
                                r"SELECT id, recipient_id, notification_type, related_answer_id,
                                    title, body, url, created_at AS `created_at!: chrono::DateTime<chrono::Utc>`,
                                    read_at
                                FROM notifications
                                WHERE recipient_id = ?
                                ORDER BY id DESC LIMIT ?",
                                recipient_id,
                                overfetch,
                            )
                            .fetch_all(&mut **txn)
                            .await?
                        }
                    };

                    Ok::<_, InfraError>(rows)
                })
            })
            .await?;

        Ok(Page::from_overfetched_items(rows, limit, |row| {
            NotificationPagePosition::new(
                Uuid::parse_str(&row.id)
                    .expect("notification IDs stored by this service are valid UUIDs")
                    .into(),
            )
        }))
    }

    #[tracing::instrument(skip_all, fields(recipient_id = %recipient_id))]
    async fn fetch_all_notifications(
        &self,
        recipient_id: UserId,
    ) -> Result<Vec<NotificationRecord>, InfraError> {
        let recipient_id = recipient_id.to_string();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                sqlx::query_as!(
                    NotificationRecord,
                    r"SELECT id, recipient_id, notification_type, related_answer_id,
                        title, body, url, created_at AS `created_at!: chrono::DateTime<chrono::Utc>`,
                        read_at
                    FROM notifications
                    WHERE recipient_id = ?
                    ORDER BY id DESC",
                    recipient_id,
                )
                .fetch_all(&mut **txn)
                .await
                .map_err(Into::<InfraError>::into)
            })
        })
        .await
    }

    #[tracing::instrument(skip_all, fields(notification_id = %notification.id()))]
    async fn update_notification(&self, notification: &Notification) -> Result<(), InfraError> {
        let id = notification.id().to_string();
        let recipient_id = notification.recipient_id().to_string();
        let read_at = *notification.read_at();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "UPDATE notifications SET read_at = ? WHERE id = ? AND recipient_id = ? AND read_at IS NULL",
                    read_at,
                    id,
                    recipient_id,
                )
                .execute(&mut **txn)
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument(skip_all)]
    async fn update_notifications(&self, notifications: &[Notification]) -> Result<(), InfraError> {
        if notifications.is_empty() {
            return Ok(());
        }

        let notifications = notifications
            .iter()
            .map(|notification| {
                (
                    notification.id().to_string(),
                    notification.recipient_id().to_string(),
                    *notification.read_at(),
                )
            })
            .collect::<Vec<_>>();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                for (id, recipient_id, read_at) in notifications {
                    sqlx::query!(
                        "UPDATE notifications SET read_at = ? WHERE id = ? AND recipient_id = ? AND read_at IS NULL",
                        read_at,
                        id,
                        recipient_id,
                    )
                    .execute(&mut **txn)
                    .await?;
                }

                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument(skip_all)]
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

    #[tracing::instrument(skip_all, fields(recipient_id = %recipient_id))]
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
