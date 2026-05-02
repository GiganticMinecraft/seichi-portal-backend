use std::str::FromStr;

use crate::{
    database::{
        components::FormCommentDatabase,
        connection::{ConnectionPool, execute_and_values},
        count::count_as_u32,
    },
    dto::{CommentDto, UserDto},
};
use async_trait::async_trait;
use domain::{
    form::{
        answer::models::AnswerId,
        comment::models::{Comment, CommentId},
    },
    user::models::Role,
};
use errors::infra::InfraError;

struct CommentReadRow {
    answer_id: String,
    comment_id: String,
    content: String,
    timestamp: chrono::DateTime<chrono::Utc>,
    commented_by_id: String,
    commented_by_name: String,
    commented_by_role: String,
}

impl CommentReadRow {
    fn into_dto(self) -> Result<CommentDto, InfraError> {
        Ok(CommentDto {
            answer_id: self.answer_id,
            comment_id: self.comment_id,
            content: self.content,
            timestamp: self.timestamp,
            commented_by: UserDto {
                name: self.commented_by_name,
                id: self.commented_by_id,
                role: Role::from_str(self.commented_by_role.as_str())?,
            },
        })
    }
}

#[async_trait]
impl FormCommentDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn get_comment(&self, comment_id: CommentId) -> Result<Option<CommentDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let comment = sqlx::query!(
                    r"SELECT form_answer_comments.id AS comment_id, answer_id, commented_by, name, role, content, timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>`
                    FROM form_answer_comments
                    INNER JOIN users ON form_answer_comments.commented_by = users.id
                    WHERE form_answer_comments.id = ?",
                    comment_id.into_inner().to_string(),
                )
                .fetch_optional(&mut **txn)
                .await?;

                comment
                    .map(|row| {
                        CommentReadRow {
                            answer_id: row.answer_id,
                            comment_id: row.comment_id,
                            content: row.content,
                            timestamp: row.timestamp,
                            commented_by_id: row.commented_by,
                            commented_by_name: row.name,
                            commented_by_role: row.role,
                        }
                        .into_dto()
                    })
                    .transpose()
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn get_comments(&self, answer_id: AnswerId) -> Result<Vec<CommentDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let comments = sqlx::query!(
                    r"SELECT form_answer_comments.id AS comment_id, answer_id, commented_by, name, role, content, timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>`
                    FROM form_answer_comments
                    INNER JOIN users ON form_answer_comments.commented_by = users.id
                    WHERE answer_id = ?",
                    answer_id.into_inner().to_string(),
                )
                .fetch_all(&mut **txn)
                .await?;

                comments
                    .into_iter()
                    .map(|row| {
                        CommentReadRow {
                            answer_id: row.answer_id,
                            comment_id: row.comment_id,
                            content: row.content,
                            timestamp: row.timestamp,
                            commented_by_id: row.commented_by,
                            commented_by_name: row.name,
                            commented_by_role: row.role,
                        }
                        .into_dto()
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn get_all_comments(&self) -> Result<Vec<CommentDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let comments = sqlx::query!(
                    r"SELECT form_answer_comments.id AS comment_id, answer_id, commented_by, name, role, content, timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>`
                    FROM form_answer_comments
                    INNER JOIN users ON form_answer_comments.commented_by = users.id"
                )
                    .fetch_all(&mut **txn)
                    .await?;

                comments
                    .into_iter()
                    .map(|row| {
                        CommentReadRow {
                            answer_id: row.answer_id,
                            comment_id: row.comment_id,
                            content: row.content,
                            timestamp: row.timestamp,
                            commented_by_id: row.commented_by,
                            commented_by_name: row.name,
                            commented_by_role: row.role,
                        }
                        .into_dto()
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn upsert_comment(
        &self,
        answer_id: AnswerId,
        comment: &Comment,
    ) -> Result<(), InfraError> {
        let params = [
            comment.comment_id().into_inner().to_string().into(),
            answer_id.into_inner().to_string().into(),
            comment.commented_by().id.to_string().into(),
            comment
                .content()
                .to_owned()
                .into_inner()
                .into_inner()
                .into(),
        ];

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    r"INSERT INTO form_answer_comments (id, answer_id, commented_by, content)
                        VALUES (?, ?, ?, ?)
                        ON DUPLICATE KEY UPDATE
                        content = VALUES(content)",
                    params,
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn delete_comment(&self, comment_id: CommentId) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "DELETE FROM form_answer_comments WHERE id = ?",
                    [comment_id.into_inner().to_string().into()],
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
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
