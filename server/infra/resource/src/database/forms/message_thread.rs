use async_trait::async_trait;
use errors::infra::InfraError;
use sqlx::Row;

use crate::database::{components::FormMessageThreadDatabase, connection::ConnectionPool};

#[async_trait]
impl FormMessageThreadDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn create_message_thread(
        &self,
        answer_id: &str,
        answer_author_id: &str,
    ) -> Result<(), InfraError> {
        let (answer_id, answer_author_id) = (answer_id.to_owned(), answer_author_id.to_owned());

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query(
                    r"INSERT INTO message_threads (answer_id, answer_author_id) VALUES (?, ?)",
                )
                .bind(answer_id)
                .bind(answer_author_id)
                .execute(&mut **txn)
                .await?;

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
                let row = sqlx::query(
                    r"SELECT answer_author_id FROM message_threads WHERE answer_id = ?",
                )
                .bind(answer_id)
                .fetch_optional(&mut **txn)
                .await?;

                row.map(|row| {
                    let answer_author_id: String = row.try_get("answer_author_id")?;
                    Ok::<_, InfraError>(answer_author_id)
                })
                .transpose()
            })
        })
        .await
    }
}
