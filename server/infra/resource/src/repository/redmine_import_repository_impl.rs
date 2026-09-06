use async_trait::async_trait;
use std::{collections::HashMap, mem};

use domain::{
    form::{
        answer::AnswerId,
        comment::CommentId,
        comment_attachment::{CommentAttachment, MAX_COMMENT_ATTACHMENTS_PER_COMMENT},
        redmine_import::{
            RedmineImportAnswerRelationsResult, RedmineImportCommentAttachmentBatch,
            RedmineImportResult, RedmineImportTarget, RedmineImportVerification,
            RedmineImportedIssue, RedmineIssueRelationBatch,
        },
    },
    repository::redmine_import_repository::RedmineImportRepository,
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Read},
};
use errors::{Error, infra::InfraError};
use sqlx::{AssertSqlSafe, query};
use uuid::Uuid;

use crate::{
    database::{connection::RedmineImportConnectionPool, redmine_import::RedmineImportDatabase},
    object_storage::{ObjectStorage, comment_attachment_object_key},
    repository::Repository,
};

#[async_trait]
impl RedmineImportRepository for Repository<RedmineImportConnectionPool> {
    async fn find_target(
        &self,
        form_id: domain::form::models::FormId,
        form_title: &str,
        label_names: &[String],
    ) -> Result<Option<AuthorizationGuard<RedmineImportTarget, Read>>, Error> {
        self.client
            .find_target(form_id, form_title, label_names)
            .await
            .map(|target| target.map(AuthorizationGuard::from))
            .map_err(Into::into)
    }

    async fn verify_issue(
        &self,
        issue: &Allowed<RedmineImportedIssue, Read>,
    ) -> Result<RedmineImportVerification, Error> {
        self.client
            .verify_issue(issue.value())
            .await
            .map_err(Into::into)
    }

    async fn import_issue(
        &self,
        issue: Allowed<RedmineImportedIssue, Create>,
    ) -> Result<RedmineImportResult, Error> {
        self.client
            .import_issue(issue.into_inner())
            .await
            .map_err(Into::into)
    }

    async fn import_comment_attachments(
        &self,
        attachments: Allowed<RedmineImportCommentAttachmentBatch, Create>,
    ) -> Result<usize, Error> {
        let pending = prepare_comment_attachments(&self.client, attachments.into_inner()).await?;
        if pending.is_empty() {
            return Ok(0);
        }

        let storage = self.object_storage.as_ref().ok_or_else(missing_storage)?;
        let mut stored_keys = Vec::with_capacity(pending.len());
        let uploaded_count = pending.len();
        let mut pending = pending;
        for item in &mut pending {
            let key = comment_attachment_object_key(*item.attachment.id());
            if let Err(error) = storage
                .put(
                    &key,
                    mem::take(&mut item.content),
                    item.attachment.content_type(),
                )
                .await
            {
                cleanup_objects(storage.as_ref(), &stored_keys).await;
                return Err(error.into());
            }
            stored_keys.push(key);
        }

        let result = self
            .client
            .read_write_transaction(|txn| {
                Box::pin(async move {
                    let placeholders = std::iter::repeat_n("(?, ?, ?, ?, ?, ?, ?)", pending.len())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let sql = format!(
                        r"INSERT INTO form_answer_comment_attachments
                            (id, answer_id, comment_id, file_name, content_type, size, created_at)
                        VALUES {placeholders}"
                    );
                    let query = pending
                        .iter()
                        .fold(query(AssertSqlSafe(&*sql)), |query, item| {
                            query
                                .bind(item.attachment.id().to_string())
                                .bind(item.attachment.answer_id().to_string())
                                .bind(item.attachment.comment_id().to_string())
                                .bind(item.attachment.file_name().as_str())
                                .bind(item.attachment.content_type())
                                .bind(*item.attachment.size())
                                .bind(item.attachment.created_at())
                        });
                    query.execute(&mut **txn).await.map_err(InfraError::from)?;
                    Ok::<_, Error>(())
                })
            })
            .await;
        if let Err(error) = result {
            cleanup_objects(storage.as_ref(), &stored_keys).await;
            return Err(error);
        }
        Ok(uploaded_count)
    }

    async fn import_answer_relations(
        &self,
        relations: Allowed<RedmineIssueRelationBatch, Create>,
    ) -> Result<RedmineImportAnswerRelationsResult, Error> {
        self.client
            .import_answer_relations(relations.into_inner())
            .await
            .map_err(Into::into)
    }
}

struct PendingCommentAttachment {
    attachment: CommentAttachment,
    content: Vec<u8>,
}

fn missing_storage() -> Error {
    InfraError::Unexpected {
        cause: "comment attachment object storage is not configured".to_string(),
    }
    .into()
}

async fn cleanup_objects(storage: &dyn ObjectStorage, keys: &[String]) {
    for key in keys {
        if let Err(error) = storage.delete(key).await {
            tracing::error!(%error, key, "failed to clean up imported Redmine comment attachment");
        }
    }
}

async fn prepare_comment_attachments(
    client: &RedmineImportConnectionPool,
    batch: RedmineImportCommentAttachmentBatch,
) -> Result<Vec<PendingCommentAttachment>, Error> {
    let (issue_id, attachments) = batch.into_parts();
    if attachments.is_empty() {
        return Ok(Vec::new());
    }

    client
        .read_only_transaction(|txn| {
            Box::pin(async move {
                let issue_row = sqlx::query!(
                    "SELECT answer_id FROM redmine_imported_answer_references WHERE redmine_issue_id = ?",
                    issue_id.into_inner(),
                )
                .fetch_optional(&mut **txn)
                .await?
                .ok_or_else(|| InfraError::Unexpected {
                    cause: format!(
                        "Redmine issue {} の移行済み回答が見つかりません",
                        issue_id.into_inner()
                    ),
                })?;
                let answer_id: AnswerId = Uuid::parse_str(&issue_row.answer_id)?.into();

                let comment_rows = sqlx::query!(
                    "SELECT redmine_journal_id, comment_id
                     FROM redmine_imported_comments
                     WHERE answer_id = ?",
                    answer_id.to_string(),
                )
                .fetch_all(&mut **txn)
                .await?;
                let comment_ids_by_journal = comment_rows
                    .into_iter()
                    .map(|row| {
                        Ok::<_, InfraError>((
                            row.redmine_journal_id,
                            Uuid::parse_str(&row.comment_id)?.into(),
                        ))
                    })
                    .collect::<Result<HashMap<_, CommentId>, _>>()?;

                let existing_rows = sqlx::query!(
                    "SELECT comment_id, file_name, size
                     FROM form_answer_comment_attachments
                     WHERE answer_id = ?",
                    answer_id.to_string(),
                )
                .fetch_all(&mut **txn)
                .await?;
                let mut existing_counts = HashMap::new();
                let mut existing_identities = HashMap::new();
                for row in existing_rows {
                    let comment_id: CommentId = Uuid::parse_str(&row.comment_id)?.into();
                    *existing_counts.entry(comment_id).or_insert(0_usize) += 1;
                    *existing_identities
                        .entry((comment_id, row.file_name, row.size))
                        .or_insert(0_usize) += 1;
                }
                let mut pending_counts: HashMap<CommentId, usize> = HashMap::new();
                let mut pending = Vec::with_capacity(attachments.len());

                for input in attachments {
                    let comment_id = comment_ids_by_journal
                        .get(&input.journal_id())
                        .copied()
                        .ok_or_else(|| InfraError::Unexpected {
                            cause: format!(
                                "Redmine issue {} の journal {} に対応する移行済みコメントが見つかりません",
                                issue_id.into_inner(),
                                input.journal_id()
                            ),
                        })?;

                    let size = u64::try_from(input.content().len()).map_err(|_| {
                        InfraError::Unexpected {
                            cause: format!(
                                "Redmine journal {} の添付サイズを変換できません",
                                input.journal_id()
                            ),
                        }
                    })?;
                    let identity = (comment_id, input.file_name().to_owned(), size);
                    if let Some(existing_count) = existing_identities.get_mut(&identity)
                        && *existing_count > 0
                    {
                        *existing_count -= 1;
                        continue;
                    }

                    let existing_count = existing_counts.get(&comment_id).copied().unwrap_or_default();
                    let new_count = existing_count
                        + pending_counts.get(&comment_id).copied().unwrap_or_default()
                        + 1;
                    if new_count > MAX_COMMENT_ATTACHMENTS_PER_COMMENT {
                        return Err(InfraError::Unexpected {
                            cause: format!(
                                "a comment must not have more than {MAX_COMMENT_ATTACHMENTS_PER_COMMENT} attachments"
                            ),
                        });
                    }
                    pending_counts.insert(comment_id, new_count - existing_count);

                    let attachment = CommentAttachment::new(
                        answer_id,
                        comment_id,
                        input.file_name().to_owned(),
                        input.content_type().to_owned(),
                        size,
                        input.created_at(),
                    )
                    .map_err(|error| InfraError::Unexpected {
                        cause: error.to_string(),
                    })?;
                    pending.push(PendingCommentAttachment {
                        attachment,
                        content: input.into_content(),
                    });
                }

                Ok(pending)
            })
        })
        .await
        .map_err(Into::into)
}
