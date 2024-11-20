use std::str::FromStr;

use async_trait::async_trait;
use domain::{
    notification::models::{Notification, NotificationId, NotificationSource},
    user::models::Role,
};
use errors::infra::InfraError;
use types::Id;
use uuid::Uuid;

use crate::{
    database::{
        components::NotificationDatabase,
        connection::{batch_insert, execute_and_values, query_all_and_values, ConnectionPool},
    },
    dto::{NotificationDto, NotificationSourceInformationDto, NotificationSourceTypeDto, UserDto},
};

#[async_trait]
impl NotificationDatabase for ConnectionPool {
    async fn create(&self, notification: &Notification) -> Result<(), InfraError> {
        let notification_source_with_id = match notification.source() {
            NotificationSource::Message(message_id) => {
                ("MESSAGE".to_owned(), message_id.to_string())
            }
        };

        let params = [
            notification.id().to_string().into(),
            notification_source_with_id.0.into(),
            notification_source_with_id.1.into(),
            notification.recipient().id.to_string().into(),
            notification.is_read().to_owned().into(),
        ];

        self.read_write_transaction(|txn| Box::pin(async move {
            execute_and_values(
                r"INSERT INTO notifications (id, source_type, source_id, recipient_id, is_read) VALUES (?, ?, ?, ?, ?)",
                params,
                txn,
            ).await?;

            Ok::<_, InfraError>(())
        })).await.map_err(Into::into)
    }

    async fn fetch_by_recipient(
        &self,
        recipient_id: Uuid,
    ) -> Result<Vec<NotificationDto>, InfraError> {
        self.read_only_transaction(|txn| Box::pin(async move {
            let rs = query_all_and_values(
                r"SELECT notifications.id AS notification_id, source_type, source_id, is_read, recipient_id, name, role
                FROM notifications
                INNER JOIN users ON notifications.recipient_id = users.id
                WHERE recipient_id = ?",
                [recipient_id.into()],
                txn,
            ).await?;

            rs.into_iter()
                .map(|rs| {
                    Ok::<_, InfraError>(NotificationDto {
                        id: uuid::Uuid::from_str(&rs.try_get::<String>("", "notification_id")?)?,
                        source: NotificationSourceInformationDto {
                            source_type: NotificationSourceTypeDto::from_str(&rs.try_get::<String>("", "source_type")?)?,
                            source_id: uuid::Uuid::from_str(&rs.try_get::<String>("", "source_id")?)?,
                        },
                        recipient: UserDto {
                            name: rs.try_get("", "name")?,
                            id: recipient_id,
                            role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                        },
                        is_read: rs.try_get("", "is_read")?,
                    })
                })
                .collect::<Result<Vec<_>, _>>()
        })).await.map_err(Into::into)
    }

    async fn fetch_by_notification_ids(
        &self,
        notification_ids: Vec<NotificationId>,
    ) -> Result<Vec<NotificationDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let rs = query_all_and_values(
                    format!(
                        r"SELECT notifications.id AS notification_id, source_type, source_id, is_read, recipient_id, name, role
                        FROM notifications
                        INNER JOIN users ON notifications.recipient_id = users.id
                        WHERE notifications.id IN (?{})",
                        ", ?".repeat(notification_ids.len() - 1)
                    )
                    .as_str(),
                    notification_ids
                        .into_iter()
                        .map(Id::into_inner)
                        .map(Into::into),
                    txn,
                )
                .await?;

                rs.into_iter()
                    .map(|rs| {
                        Ok::<_, InfraError>(NotificationDto {
                            id: uuid::Uuid::from_str(
                                &rs.try_get::<String>("", "notification_id")?,
                            )?,
                            source: NotificationSourceInformationDto {
                                source_type: NotificationSourceTypeDto::from_str(
                                    &rs.try_get::<String>("", "source_type")?,
                                )?,
                                source_id: uuid::Uuid::from_str(
                                    &rs.try_get::<String>("", "source_id")?,
                                )?,
                            },
                            recipient: UserDto {
                                name: rs.try_get("", "name")?,
                                id: uuid::Uuid::from_str(
                                    &rs.try_get::<String>("", "recipient_id")?,
                                )?,
                                role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                            },
                            is_read: rs.try_get("", "is_read")?,
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn update_read_status(
        &self,
        notification_id_with_is_read: Vec<(NotificationId, bool)>,
    ) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                batch_insert(
                    r"INSERT INTO notifications (id, is_read) VALUES (?, ?) ON DUPLICATE KEY UPDATE is_read = VALUES(is_read)",
                        notification_id_with_is_read.into_iter().flat_map(|(id, is_read)| [id.to_string().into(), is_read.into()]),
                    txn,
                ).await?;

                Ok::<_, InfraError>(())
            })
        }).await.map_err(Into::into)
    }
}
