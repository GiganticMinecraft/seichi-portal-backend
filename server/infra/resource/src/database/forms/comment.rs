use crate::{
    database::{components::FormCommentDatabase, connection::ConnectionPool, count::count_as_u32},
    records::{CommentHistoryRecord, CommentRecord},
};
use async_trait::async_trait;
use domain::form::{
    answer::AnswerId,
    comment::{Comment, CommentHistoryPagePosition, CommentId},
};
use domain::{
    account::models::AccountUser,
    pagination::{Page, PageRequest},
};
use errors::infra::InfraError;
use uuid::Uuid;

#[async_trait]
impl FormCommentDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn get_comment(
        &self,
        comment_id: CommentId,
    ) -> Result<Option<CommentRecord>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let comment = sqlx::query_as!(
                    CommentRecord,
                    r"SELECT form_answer_comments.id AS comment_id, answer_id, commented_by AS commented_by_id, name AS commented_by_name, role AS commented_by_role, content, timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>`
                    FROM form_answer_comments
                    INNER JOIN users ON form_answer_comments.commented_by = users.id
                    WHERE form_answer_comments.id = ?",
                    comment_id.into_inner().to_string(),
                )
                .fetch_optional(&mut **txn)
                .await?;

                Ok::<_, InfraError>(comment)
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn get_comments(&self, answer_id: AnswerId) -> Result<Vec<CommentRecord>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let comments = sqlx::query_as!(
                    CommentRecord,
                    r"SELECT form_answer_comments.id AS comment_id, answer_id, commented_by AS commented_by_id, name AS commented_by_name, role AS commented_by_role, content, timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>`
                    FROM form_answer_comments
                    INNER JOIN users ON form_answer_comments.commented_by = users.id
                    WHERE answer_id = ?",
                    answer_id.into_inner().to_string(),
                )
                .fetch_all(&mut **txn)
                .await?;

                Ok::<_, InfraError>(comments)
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn get_all_comments(&self) -> Result<Vec<CommentRecord>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let comments = sqlx::query_as!(
                    CommentRecord,
                    r"SELECT form_answer_comments.id AS comment_id, answer_id, commented_by AS commented_by_id, name AS commented_by_name, role AS commented_by_role, content, timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>`
                    FROM form_answer_comments
                    INNER JOIN users ON form_answer_comments.commented_by = users.id"
                )
                .fetch_all(&mut **txn)
                .await?;

                Ok::<_, InfraError>(comments)
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn create_comment(&self, comment: &Comment) -> Result<(), InfraError> {
        let comment_id = comment.comment_id().into_inner().to_string();
        let answer_id = comment.answer_id().into_inner().to_string();
        let commented_by = comment.commented_by().to_string();
        let content = comment.content().to_owned().into_inner().into_inner();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "INSERT INTO form_answer_comments (id, answer_id, commented_by, content) VALUES (?, ?, ?, ?)",
                    comment_id,
                    answer_id,
                    commented_by,
                    content,
                )
                .execute(&mut **txn)
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn update_comment_with_history(
        &self,
        comment: &Comment,
        operated_by: &AccountUser,
    ) -> Result<(), InfraError> {
        let comment_id = comment.comment_id().to_string();
        let answer_id = comment.answer_id().to_string();
        let new_content = comment.content().to_string();
        let operator_id = operated_by.id().to_string();
        let operator_name = operated_by.name().to_owned();
        let operator_role = operated_by.role().to_string();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                let current = sqlx::query!(
                    r"SELECT c.answer_id, c.commented_by AS original_author_id,
                        u.name AS original_author_name, u.role AS original_author_role,
                        c.content, c.timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>`
                    FROM form_answer_comments c
                    INNER JOIN users u ON u.id = c.commented_by
                    WHERE c.id = ? FOR UPDATE",
                    comment_id,
                )
                .fetch_optional(&mut **txn)
                .await?;

                let current = current.ok_or_else(|| InfraError::Unexpected {
                    cause: format!("comment {comment_id} was not found while updating"),
                })?;
                if current.answer_id != answer_id {
                    return Err(InfraError::Unexpected {
                        cause: format!("comment {comment_id} does not belong to answer {answer_id}"),
                    });
                }
                if current.content == new_content {
                    return Ok(());
                }

                let history_id = Uuid::now_v7().to_string();
                sqlx::query!(
                    r"INSERT INTO form_answer_comment_history
                    (id, answer_id, comment_id, original_author_id, original_author_name,
                     original_author_role, original_timestamp, action, before_content, after_content,
                     operated_by_id, operated_by_name, operated_by_role)
                    VALUES (?, ?, ?, ?, ?, ?, ?, 'UPDATE', ?, ?, ?, ?, ?)",
                    history_id, answer_id, comment_id, current.original_author_id,
                    current.original_author_name, current.original_author_role,
                    current.timestamp, current.content, new_content, operator_id, operator_name,
                    operator_role,
                )
                .execute(&mut **txn)
                .await?;
                let result = sqlx::query!(
                    "UPDATE form_answer_comments SET content = ? WHERE id = ?",
                    new_content,
                    comment_id,
                )
                .execute(&mut **txn)
                .await?;
                if result.rows_affected() != 1 {
                    return Err(InfraError::Unexpected {
                        cause: format!("comment {comment_id} update affected an unexpected number of rows"),
                    });
                }

                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument(skip(self, operated_by))]
    async fn delete_comment_with_history(
        &self,
        comment_id: CommentId,
        operated_by: &AccountUser,
    ) -> Result<(), InfraError> {
        let comment_id = comment_id.to_string();
        let operator_id = operated_by.id().to_string();
        let operator_name = operated_by.name().to_owned();
        let operator_role = operated_by.role().to_string();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                let current = sqlx::query!(
                    r"SELECT c.answer_id, c.commented_by AS original_author_id,
                    u.name AS original_author_name, u.role AS original_author_role,
                    c.timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>`
                FROM form_answer_comments c
                INNER JOIN users u ON u.id = c.commented_by
                WHERE c.id = ? FOR UPDATE",
                    comment_id,
                )
                .fetch_optional(&mut **txn)
                .await?
                .ok_or_else(|| InfraError::Unexpected {
                    cause: format!("comment {comment_id} was not found while deleting"),
                })?;

                let history_id = Uuid::now_v7().to_string();
                sqlx::query!(
                    r"INSERT INTO form_answer_comment_history
                (id, answer_id, comment_id, original_author_id, original_author_name,
                 original_author_role, original_timestamp, action,
                 operated_by_id, operated_by_name, operated_by_role)
                VALUES (?, ?, ?, ?, ?, ?, ?, 'DELETE', ?, ?, ?)",
                    history_id,
                    current.answer_id,
                    comment_id,
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
                let result =
                    sqlx::query!("DELETE FROM form_answer_comments WHERE id = ?", comment_id)
                        .execute(&mut **txn)
                        .await?;
                if result.rows_affected() != 1 {
                    return Err(InfraError::Unexpected {
                        cause: format!(
                            "comment {comment_id} delete affected an unexpected number of rows"
                        ),
                    });
                }
                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument(skip(self))]
    async fn get_history(
        &self,
        answer_id: AnswerId,
        request: PageRequest<CommentHistoryPagePosition>,
    ) -> Result<Page<CommentHistoryRecord, CommentHistoryPagePosition>, InfraError> {
        let answer_id = answer_id.to_string();
        let after = request
            .after_position()
            .map(|position| position.id().to_string());
        let limit = request.limit();
        let rows = self.read_only_transaction(|txn| Box::pin(async move {
            sqlx::query_as!(
                CommentHistoryRecord,
                r"SELECT id, answer_id, comment_id, original_author_id, original_author_name,
                    original_author_role, original_timestamp AS `original_timestamp!: chrono::DateTime<chrono::Utc>`,
                    action, before_content, after_content, operated_by_id, operated_by_name,
                    operated_by_role, operated_at AS `operated_at!: chrono::DateTime<chrono::Utc>`
                FROM form_answer_comment_history
                WHERE answer_id = ? AND (? IS NULL OR id < ?)
                ORDER BY id DESC LIMIT ?",
                answer_id,
                after,
                after,
                request.limit().overfetch_value(),
            ).fetch_all(&mut **txn).await.map_err(Into::<InfraError>::into)
        })).await?;

        Ok(Page::from_overfetched_items(rows, limit, |row| {
            CommentHistoryPagePosition::new(
                Uuid::parse_str(&row.id)
                    .expect("history IDs stored by this service are valid UUIDs")
                    .into(),
            )
        }))
    }

    #[tracing::instrument]
    async fn size(&self) -> Result<u32, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let size = sqlx::query_scalar!(
                    "SELECT COUNT(*) AS `count!: i64` FROM form_answer_comments"
                )
                .fetch_one(&mut **txn)
                .await?;

                count_as_u32(size, "form_answer_comments")
            })
        })
        .await
    }
}
