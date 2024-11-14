use async_trait::async_trait;
use domain::notification::models::{Notification, NotificationSource};
use errors::infra::InfraError;

use crate::database::{
    components::NotificationDatabase,
    connection::{execute_and_values, ConnectionPool},
};

#[async_trait]
impl NotificationDatabase for ConnectionPool {
    async fn create(&self, notification: &Notification) -> Result<(), InfraError> {
        let notification_source_with_id = match notification.source() {
            NotificationSource::Message { message_id } => {
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
                txn
            ).await?;

            Ok::<_, InfraError>(())
        })).await.map_err(Into::into)
    }
}
