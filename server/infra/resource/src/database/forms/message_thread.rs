use async_trait::async_trait;
use domain::{account::models::UserSnapshot, form::message_thread::MessageThread};
use errors::infra::InfraError;
use uuid::Uuid;

use crate::database::{components::FormMessageThreadDatabase, connection::ConnectionPool};

#[async_trait]
impl FormMessageThreadDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn create_message_thread(
        &self,
        message_thread: &MessageThread,
        operated_by: &UserSnapshot,
    ) -> Result<(), InfraError> {
        let answer_id = message_thread.answer_id().to_string();
        let answer_author_id = message_thread.answer_author_id().to_string();
        let operator_id = operated_by.id().to_string();
        let operator_name = operated_by.name().to_owned();
        let operator_role = operated_by.role().to_string();
        let messages = message_thread
            .messages()
            .iter()
            .map(|message| {
                (
                    message.id().to_string(),
                    message.sender_id().to_string(),
                    message.body().as_str().to_owned(),
                    *message.timestamp(),
                )
            })
            .collect::<Vec<_>>();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "INSERT INTO message_threads (answer_id, answer_author_id) VALUES (?, ?)",
                    answer_id,
                    answer_author_id,
                )
                .execute(&mut **txn)
                .await?;

                for (message_id, sender_id, body, timestamp) in messages {
                    sqlx::query!(
                        "INSERT INTO messages (id, related_answer_id, sender, body, timestamp) VALUES (?, ?, ?, ?, ?)",
                        message_id,
                        answer_id,
                        sender_id,
                        body,
                        timestamp,
                    )
                    .execute(&mut **txn)
                    .await?;

                    sqlx::query!(
                        r"INSERT INTO message_history
                        (id, related_answer_id, message_id, original_author_id, original_author_name,
                         original_author_role, original_timestamp, action, body,
                         operated_by_id, operated_by_name, operated_by_role, operated_at)
                        VALUES (?, ?, ?, ?, ?, ?, ?, 'CREATE', ?, ?, ?, ?, ?)",
                        Uuid::now_v7().to_string(),
                        answer_id,
                        message_id,
                        sender_id,
                        operator_name,
                        operator_role,
                        timestamp,
                        body,
                        operator_id,
                        operator_name,
                        operator_role,
                        timestamp,
                    )
                    .execute(&mut **txn)
                    .await?;
                }

                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn get_thread_author_by_answer_id(
        &self,
        answer_id: &str,
    ) -> Result<Option<String>, InfraError> {
        let answer_id = answer_id.to_owned();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let answer_author_id = sqlx::query_scalar!(
                    "SELECT answer_author_id FROM message_threads WHERE answer_id = ?",
                    answer_id,
                )
                .fetch_optional(&mut **txn)
                .await?;

                Ok::<_, InfraError>(answer_author_id)
            })
        })
        .await
    }
}
