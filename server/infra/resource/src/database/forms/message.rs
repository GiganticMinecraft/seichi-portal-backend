use async_trait::async_trait;
use domain::form::{
    answer::models::AnswerEntry,
    message::models::{Message, MessageId},
};
use errors::infra::InfraError;

use crate::{
    database::{components::FormMessageDatabase, connection::ConnectionPool},
    dto::MessageDto,
};

#[async_trait]
impl FormMessageDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn post_message(&self, message: &Message) -> Result<(), InfraError> {
        let id = message.id().to_string().to_owned();
        let related_answer_id = message.related_answer_id().to_string();
        let sender = message.sender_id().to_string();
        let body = message.body().to_owned();
        let timestamp = message.timestamp().to_owned();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    r"INSERT INTO messages (id, related_answer_id, sender, body, timestamp) VALUES (?, ?, ?, ?, ?)",
                    id,
                    related_answer_id,
                    sender,
                    body,
                    timestamp,
                )
                .execute(&mut **txn)
                .await?;

                Ok::<_, InfraError>(())
            })
        }).await
    }

    #[tracing::instrument]
    async fn update_message_body(
        &self,
        message_id: MessageId,
        body: String,
    ) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "UPDATE messages SET body = ? WHERE id = ?",
                    body,
                    message_id.to_string(),
                )
                .execute(&mut **txn)
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn fetch_messages_by_form_answer(
        &self,
        answers: &AnswerEntry,
    ) -> Result<Vec<MessageDto>, InfraError> {
        let answer_id = answers.id().into_inner().to_owned();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let rows = sqlx::query_as!(
                    MessageDto,
                    r"SELECT messages.id AS id, related_answer_id AS related_answer, sender AS sender_id, users.name AS sender_name, users.role AS sender_role, body, timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>`
                    FROM messages
                    INNER JOIN users ON users.id = messages.sender
                    WHERE related_answer_id = ?",
                    answer_id.to_string(),
                )
                .fetch_all(&mut **txn)
                .await?;

                Ok::<_, InfraError>(rows)
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn fetch_message(
        &self,
        message_id: &MessageId,
    ) -> Result<Option<MessageDto>, InfraError> {
        let message_id = message_id.into_inner().to_string();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let row = sqlx::query_as!(
                    MessageDto,
                    r"SELECT messages.id AS id, related_answer_id AS related_answer, sender AS sender_id, users.name AS sender_name, users.role AS sender_role, body, timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>`
                    FROM messages
                    INNER JOIN users ON users.id = messages.sender
                    WHERE messages.id = ?",
                    message_id,
                )
                .fetch_optional(&mut **txn)
                .await?;

                Ok::<_, InfraError>(row)
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn delete_message(&self, message_id: MessageId) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!("DELETE FROM messages WHERE id = ?", message_id.to_string(),)
                    .execute(&mut **txn)
                    .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
    }
}
