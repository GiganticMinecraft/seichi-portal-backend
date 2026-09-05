use async_trait::async_trait;
use domain::form::{
    comment::CommentId,
    comment_attachment::{
        CommentAttachment, CommentAttachmentId, MAX_COMMENT_ATTACHMENTS_PER_COMMENT,
    },
};
use errors::{Error, domain::DomainError, infra::InfraError};

use crate::{
    database::{components::FormCommentAttachmentDatabase, connection::ConnectionPool},
    records::CommentAttachmentRecord,
};

#[async_trait]
impl FormCommentAttachmentDatabase for ConnectionPool {
    async fn get_by_comment(
        &self,
        comment_id: CommentId,
    ) -> Result<Vec<CommentAttachmentRecord>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                sqlx::query_as!(
                    CommentAttachmentRecord,
                    r"SELECT id, answer_id, comment_id, file_name, content_type, size,
                        created_at AS `created_at!: chrono::DateTime<chrono::Utc>`
                    FROM form_answer_comment_attachments
                    WHERE comment_id = ?
                    ORDER BY created_at ASC, id ASC",
                    comment_id.to_string(),
                )
                .fetch_all(&mut **txn)
                .await
                .map_err(InfraError::from)
            })
        })
        .await
    }

    async fn get(
        &self,
        attachment_id: CommentAttachmentId,
    ) -> Result<Option<CommentAttachmentRecord>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                sqlx::query_as!(
                    CommentAttachmentRecord,
                    r"SELECT id, answer_id, comment_id, file_name, content_type, size,
                        created_at AS `created_at!: chrono::DateTime<chrono::Utc>`
                    FROM form_answer_comment_attachments
                    WHERE id = ?",
                    attachment_id.to_string(),
                )
                .fetch_optional(&mut **txn)
                .await
                .map_err(InfraError::from)
            })
        })
        .await
    }

    async fn create_many(&self, attachments: Vec<CommentAttachment>) -> Result<(), Error> {
        let Some(first) = attachments.first() else {
            return Ok(());
        };
        let comment_id = first.comment_id().to_string();
        let answer_id = first.answer_id().to_string();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "SELECT id FROM form_answer_comments
                     WHERE id = ? AND answer_id = ? FOR UPDATE",
                    &comment_id,
                    &answer_id,
                )
                .fetch_optional(&mut **txn)
                .await
                .map_err(InfraError::from)?
                .ok_or(DomainError::NotFound)?;

                let existing_count = sqlx::query_scalar!(
                    "SELECT COUNT(*) AS `count!: i64`
                     FROM form_answer_comment_attachments WHERE comment_id = ?",
                    &comment_id,
                )
                .fetch_one(&mut **txn)
                .await
                .map_err(InfraError::from)?;
                let new_count = existing_count
                    .checked_add(i64::try_from(attachments.len()).map_err(|_| {
                        DomainError::InvalidEntity {
                            message: "too many comment attachments".to_string(),
                        }
                    })?)
                    .ok_or_else(|| DomainError::InvalidEntity {
                        message: "too many comment attachments".to_string(),
                    })?;
                if new_count > MAX_COMMENT_ATTACHMENTS_PER_COMMENT as i64 {
                    return Err(DomainError::InvalidEntity {
                        message: format!(
                            "a comment must not have more than {MAX_COMMENT_ATTACHMENTS_PER_COMMENT} attachments"
                        ),
                    }
                    .into());
                }

                for attachment in attachments {
                    sqlx::query!(
                        r"INSERT INTO form_answer_comment_attachments
                            (id, answer_id, comment_id, file_name, content_type, size, created_at)
                        VALUES (?, ?, ?, ?, ?, ?, ?)",
                        attachment.id().to_string(),
                        attachment.answer_id().to_string(),
                        attachment.comment_id().to_string(),
                        attachment.file_name().as_str(),
                        attachment.content_type(),
                        attachment.size(),
                        attachment.created_at(),
                    )
                    .execute(&mut **txn)
                    .await
                    .map_err(InfraError::from)?;
                }
                Ok(())
            })
        })
        .await
    }

    async fn delete(&self, attachment_id: CommentAttachmentId) -> Result<(), Error> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "DELETE FROM form_answer_comment_attachments WHERE id = ?",
                    attachment_id.to_string(),
                )
                .execute(&mut **txn)
                .await
                .map_err(InfraError::from)?;
                Ok(())
            })
        })
        .await
    }

    async fn delete_for_comment(&self, comment_id: CommentId) -> Result<(), Error> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "DELETE FROM form_answer_comment_attachments WHERE comment_id = ?",
                    comment_id.to_string(),
                )
                .execute(&mut **txn)
                .await
                .map_err(InfraError::from)?;
                Ok(())
            })
        })
        .await
    }
}
