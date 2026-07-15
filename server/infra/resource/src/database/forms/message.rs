use async_trait::async_trait;
use domain::form::{
    answer::{AnswerEntry, AnswerId},
    message::{Message, MessageHistoryPagePosition, MessageId},
};
use domain::{
    account::models::AccountUser,
    pagination::{Page, PageRequest},
};
use errors::infra::InfraError;
use uuid::Uuid;

use crate::{
    database::{components::FormMessageDatabase, connection::ConnectionPool},
    records::{MessageHistoryRecord, MessageRecord},
};

#[async_trait]
impl FormMessageDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn post_message(&self, message: &Message, answer_id: AnswerId) -> Result<(), InfraError> {
        let id = message.id().to_string().to_owned();
        let related_answer_id = answer_id.into_inner().to_string();
        let sender = message.sender_id().to_string();
        let body = message.body().as_str().to_owned();
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
    async fn update_message_with_history(
        &self,
        message: &Message,
        operated_by: &AccountUser,
    ) -> Result<(), InfraError> {
        let message_id = message.id().to_string();
        let new_body = message.body().as_str().to_owned();
        let operator_id = operated_by.id().to_string();
        let operator_name = operated_by.name().to_owned();
        let operator_role = operated_by.role().to_string();
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                let current = sqlx::query!(
                    r"SELECT m.related_answer_id, m.sender AS original_author_id,
                        u.name AS original_author_name, u.role AS original_author_role,
                        m.body, m.timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>`
                    FROM messages m INNER JOIN users u ON u.id = m.sender
                    WHERE m.id = ? FOR UPDATE",
                    message_id,
                )
                .fetch_optional(&mut **txn)
                .await?
                .ok_or_else(|| InfraError::Unexpected {
                    cause: format!("message {message_id} was not found while updating"),
                })?;
                if current.body == new_body {
                    return Ok(());
                }

                let history_id = Uuid::now_v7().to_string();
                sqlx::query!(
                    r"INSERT INTO message_history
                    (id, related_answer_id, message_id, original_author_id, original_author_name,
                     original_author_role, original_timestamp, action, before_body, after_body,
                     operated_by_id, operated_by_name, operated_by_role)
                    VALUES (?, ?, ?, ?, ?, ?, ?, 'UPDATE', ?, ?, ?, ?, ?)",
                    history_id,
                    current.related_answer_id,
                    message_id,
                    current.original_author_id,
                    current.original_author_name,
                    current.original_author_role,
                    current.timestamp,
                    current.body,
                    new_body,
                    operator_id,
                    operator_name,
                    operator_role,
                )
                .execute(&mut **txn)
                .await?;
                let result = sqlx::query!(
                    "UPDATE messages SET body = ? WHERE id = ?",
                    new_body,
                    message_id
                )
                .execute(&mut **txn)
                .await?;
                if result.rows_affected() != 1 {
                    return Err(InfraError::Unexpected {
                        cause: format!(
                            "message {message_id} update affected an unexpected number of rows"
                        ),
                    });
                }

                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn fetch_messages_by_form_answer(
        &self,
        answers: &AnswerEntry,
    ) -> Result<Vec<MessageRecord>, InfraError> {
        let answer_id = answers.id().into_inner().to_owned();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let rows = sqlx::query_as!(
                    MessageRecord,
                    r"SELECT messages.id AS id, sender AS sender_id, users.name AS sender_name, users.role AS sender_role, body, timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>`
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
    async fn fetch_messages_by_answer_id(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<MessageRecord>, InfraError> {
        let answer_id = answer_id.into_inner().to_string();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let rows = sqlx::query_as!(
                    MessageRecord,
                    r"SELECT messages.id AS id, sender AS sender_id, users.name AS sender_name, users.role AS sender_role, body, timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>`
                    FROM messages
                    INNER JOIN users ON users.id = messages.sender
                    WHERE related_answer_id = ?",
                    answer_id,
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
    ) -> Result<Option<MessageRecord>, InfraError> {
        let message_id = message_id.into_inner().to_string();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let row = sqlx::query_as!(
                    MessageRecord,
                    r"SELECT messages.id AS id, sender AS sender_id, users.name AS sender_name, users.role AS sender_role, body, timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>`
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
    async fn delete_message_with_history(
        &self,
        message_id: MessageId,
        operated_by: &AccountUser,
    ) -> Result<(), InfraError> {
        let message_id = message_id.to_string();
        let operator_id = operated_by.id().to_string();
        let operator_name = operated_by.name().to_owned();
        let operator_role = operated_by.role().to_string();
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                let current = sqlx::query!(
                    r"SELECT m.related_answer_id, m.sender AS original_author_id,
                        u.name AS original_author_name, u.role AS original_author_role,
                        m.timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>`
                    FROM messages m INNER JOIN users u ON u.id = m.sender
                    WHERE m.id = ? FOR UPDATE",
                    message_id,
                )
                .fetch_optional(&mut **txn)
                .await?
                .ok_or_else(|| InfraError::Unexpected {
                    cause: format!("message {message_id} was not found while deleting"),
                })?;

                let history_id = Uuid::now_v7().to_string();
                sqlx::query!(
                    r"INSERT INTO message_history
                    (id, related_answer_id, message_id, original_author_id, original_author_name,
                     original_author_role, original_timestamp, action,
                     operated_by_id, operated_by_name, operated_by_role)
                    VALUES (?, ?, ?, ?, ?, ?, ?, 'DELETE', ?, ?, ?)",
                    history_id,
                    current.related_answer_id,
                    message_id,
                    current.original_author_id,
                    current.original_author_name,
                    current.original_author_role,
                    current.timestamp,
                    operator_id,
                    operator_name,
                    operator_role,
                )
                .execute(&mut **txn)
                .await?;
                let result = sqlx::query!("DELETE FROM messages WHERE id = ?", message_id)
                    .execute(&mut **txn)
                    .await?;
                if result.rows_affected() != 1 {
                    return Err(InfraError::Unexpected {
                        cause: format!(
                            "message {message_id} delete affected an unexpected number of rows"
                        ),
                    });
                }

                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    async fn fetch_history(
        &self,
        answer_id: AnswerId,
        request: PageRequest<MessageHistoryPagePosition>,
    ) -> Result<Page<MessageHistoryRecord, MessageHistoryPagePosition>, InfraError> {
        let answer_id = answer_id.to_string();
        let after = request
            .after_position()
            .map(|position| position.id().to_string());
        let limit = request.limit();
        let overfetch = limit.overfetch_value();
        let rows = match after {
            Some(after) => {
                self.read_only_transaction(|txn| Box::pin(async move {
                    sqlx::query_as!(
                        MessageHistoryRecord,
                        r"SELECT id, related_answer_id AS answer_id, message_id, original_author_id, original_author_name,
                            original_author_role, original_timestamp AS `original_timestamp!: chrono::DateTime<chrono::Utc>`,
                            action, before_body, after_body, operated_by_id, operated_by_name,
                            operated_by_role, operated_at AS `operated_at!: chrono::DateTime<chrono::Utc>`
                        FROM message_history
                        WHERE related_answer_id = ? AND id < ?
                        ORDER BY id DESC LIMIT ?",
                        answer_id,
                        after,
                        overfetch,
                    )
                    .fetch_all(&mut **txn)
                    .await
                    .map_err(Into::<InfraError>::into)
                }))
                .await?
            }
            None => {
                self.read_only_transaction(|txn| Box::pin(async move {
                    sqlx::query_as!(
                        MessageHistoryRecord,
                        r"SELECT id, related_answer_id AS answer_id, message_id, original_author_id, original_author_name,
                            original_author_role, original_timestamp AS `original_timestamp!: chrono::DateTime<chrono::Utc>`,
                            action, before_body, after_body, operated_by_id, operated_by_name,
                            operated_by_role, operated_at AS `operated_at!: chrono::DateTime<chrono::Utc>`
                        FROM message_history
                        WHERE related_answer_id = ?
                        ORDER BY id DESC LIMIT ?",
                        answer_id,
                        overfetch,
                    )
                    .fetch_all(&mut **txn)
                    .await
                    .map_err(Into::<InfraError>::into)
                }))
                .await?
            }
        };

        Ok(Page::from_overfetched_items(rows, limit, |row| {
            MessageHistoryPagePosition::new(
                Uuid::parse_str(&row.id)
                    .expect("history IDs stored by this service are valid UUIDs")
                    .into(),
            )
        }))
    }
}
