use crate::{
    database::{
        components::FormCommentDatabase,
        connection::{ConnectionPool, DatabaseTransaction},
        count::count_as_u32,
    },
    records::{CommentHistoryRecord, CommentRecord},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    account::models::UserGroupId,
    pagination::{Page, PageRequest},
};
use domain::{
    auth::Actor,
    form::{
        answer::{AnswerId, AnswerPublication, AnswerSettings, AnswerVisibility},
        comment::{Comment, CommentHistoryPagePosition, CommentId, DeletedComment},
        comment_thread::CommentThread,
        models::{ActiveForm, FormId, active_form_allows_read},
        settings::{AllowedUserGroups, Visibility},
    },
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Read, Update},
};
use errors::{Error, infra::InfraError};
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
    async fn create_comment_authorizing_in_transaction(
        &self,
        form: &Allowed<ActiveForm, Read>,
        comment: Allowed<Comment, Create>,
    ) -> Result<(), Error> {
        let form_id = *form.id();
        let actor = comment.actor().clone();
        let candidate = comment.into_inner();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                let thread =
                    lock_comment_thread(txn, form_id, actor, *candidate.answer_id(), None).await?;
                let comment = thread.authorize_comment_create(candidate)?.into_inner();
                insert_created_comment(txn, &comment, thread.actor()).await
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn update_comment_authorizing_in_transaction(
        &self,
        form: &Allowed<ActiveForm, Read>,
        comment: Allowed<Comment, Update>,
        operated_at: DateTime<Utc>,
    ) -> Result<(), Error> {
        let form_id = *form.id();
        let actor = comment.actor().clone();
        let candidate = comment.into_inner();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                let thread = lock_comment_thread(
                    txn,
                    form_id,
                    actor,
                    *candidate.answer_id(),
                    Some(*candidate.comment_id()),
                )
                .await?;
                let comment = thread
                    .authorize_comment_update(*candidate.comment_id(), candidate.content().clone())?
                    .into_inner();
                update_comment_with_history(txn, &comment, thread.actor(), operated_at).await
            })
        })
        .await
    }

    #[tracing::instrument(skip(self, comment))]
    async fn delete_comment_authorizing_in_transaction(
        &self,
        form: &Allowed<ActiveForm, Read>,
        comment: Allowed<DeletedComment, Create>,
    ) -> Result<(), Error> {
        let form_id = *form.id();
        let actor = comment.actor().clone();
        let candidate = comment.into_inner();
        let candidate_comment = candidate.comment();
        let answer_id = *candidate_comment.answer_id();
        let comment_id = *candidate_comment.comment_id();
        let deleted_at = *candidate.deleted_at();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                let thread =
                    lock_comment_thread(txn, form_id, actor, answer_id, Some(comment_id)).await?;
                let deleted = thread
                    .authorize_comment_delete(comment_id, deleted_at)?
                    .into_inner();
                delete_comment_with_history(txn, &deleted).await
            })
        })
        .await
    }

    #[tracing::instrument(skip(self))]
    async fn get_history(
        &self,
        answer_id: AnswerId,
        request: PageRequest<CommentHistoryPagePosition>,
        includes_deleted_history: bool,
    ) -> Result<Page<CommentHistoryRecord, CommentHistoryPagePosition>, InfraError> {
        let answer_id = answer_id.to_string();
        let after = request
            .after_position()
            .map(|position| position.id().to_string());
        let limit = request.limit();
        let overfetch = limit.overfetch_value();
        let rows = match (includes_deleted_history, after) {
            (true, Some(after)) => {
                self.read_only_transaction(|txn| Box::pin(async move {
                    sqlx::query_as!(
                        CommentHistoryRecord,
                        r"SELECT id, answer_id, comment_id, original_author_id, original_author_name,
                            original_author_role, original_timestamp AS `original_timestamp!: chrono::DateTime<chrono::Utc>`,
                            action, content, operated_by_id, operated_by_name,
                            operated_by_role, operated_at AS `operated_at!: chrono::DateTime<chrono::Utc>`
                        FROM form_answer_comment_history
                        WHERE answer_id = ? AND id < ?
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
            (true, None) => {
                self.read_only_transaction(|txn| Box::pin(async move {
                    sqlx::query_as!(
                        CommentHistoryRecord,
                        r"SELECT id, answer_id, comment_id, original_author_id, original_author_name,
                            original_author_role, original_timestamp AS `original_timestamp!: chrono::DateTime<chrono::Utc>`,
                            action, content, operated_by_id, operated_by_name,
                            operated_by_role, operated_at AS `operated_at!: chrono::DateTime<chrono::Utc>`
                        FROM form_answer_comment_history
                        WHERE answer_id = ?
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
            (false, Some(after)) => {
                self.read_only_transaction(|txn| Box::pin(async move {
                    sqlx::query_as!(
                        CommentHistoryRecord,
                        r"SELECT id, answer_id, comment_id, original_author_id, original_author_name,
                            original_author_role, original_timestamp AS `original_timestamp!: chrono::DateTime<chrono::Utc>`,
                            action, content, operated_by_id, operated_by_name,
                            operated_by_role, operated_at AS `operated_at!: chrono::DateTime<chrono::Utc>`
                        FROM form_answer_comment_history
                        WHERE answer_id = ? AND action != 'DELETE' AND id < ?
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
            (false, None) => {
                self.read_only_transaction(|txn| Box::pin(async move {
                    sqlx::query_as!(
                        CommentHistoryRecord,
                        r"SELECT id, answer_id, comment_id, original_author_id, original_author_name,
                            original_author_role, original_timestamp AS `original_timestamp!: chrono::DateTime<chrono::Utc>`,
                            action, content, operated_by_id, operated_by_name,
                            operated_by_role, operated_at AS `operated_at!: chrono::DateTime<chrono::Utc>`
                        FROM form_answer_comment_history
                        WHERE answer_id = ? AND action != 'DELETE'
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

/// フォーム、回答、更新・削除対象コメントをこの順序でロックして最新の Thread を再構成する。
async fn lock_comment_thread(
    transaction: &mut DatabaseTransaction,
    form_id: FormId,
    actor: Actor,
    answer_id: AnswerId,
    comment_id: Option<CommentId>,
) -> Result<Allowed<CommentThread, Update>, Error> {
    let answer_id_text = answer_id.to_string();
    let form_id = form_id.to_string();
    let locked_form = sqlx::query!(
        "SELECT visibility, answer_visibility FROM form_meta_data WHERE id = ? FOR UPDATE",
        form_id,
    )
    .fetch_optional(&mut **transaction)
    .await
    .map_err(InfraError::from)?
    .ok_or(errors::domain::DomainError::NotFound)?;
    let form_visibility = Visibility::try_from(locked_form.visibility)?;
    let answer_visibility = AnswerVisibility::try_from(locked_form.answer_visibility)?;

    let form_group_ids = sqlx::query!(
        "SELECT group_id FROM form_allowed_user_groups WHERE form_id = ? ORDER BY id ASC",
        form_id,
    )
    .fetch_all(&mut **transaction)
    .await
    .map_err(InfraError::from)?
    .into_iter()
    .map(|row| {
        Uuid::parse_str(&row.group_id)
            .map(UserGroupId::from)
            .map_err(InfraError::from)
    })
    .collect::<Result<Vec<_>, _>>()?;
    if !active_form_allows_read(
        &form_visibility,
        &AllowedUserGroups::new(form_group_ids),
        &actor,
    ) {
        return Err(errors::domain::DomainError::Forbidden.into());
    }

    let answer_group_ids = sqlx::query!(
        "SELECT group_id FROM form_answer_groups WHERE form_id = ? ORDER BY id ASC",
        form_id,
    )
    .fetch_all(&mut **transaction)
    .await
    .map_err(InfraError::from)?
    .into_iter()
    .map(|row| {
        Uuid::parse_str(&row.group_id)
            .map(UserGroupId::from)
            .map_err(InfraError::from)
    })
    .collect::<Result<Vec<_>, _>>()?;
    let answer = sqlx::query!(
        "SELECT publication FROM answers WHERE id = ? AND form_id = ? FOR UPDATE",
        answer_id_text,
        form_id,
    )
    .fetch_optional(&mut **transaction)
    .await
    .map_err(InfraError::from)?
    .ok_or(errors::domain::DomainError::NotFound)?;

    let comments = match comment_id {
        Some(comment_id) => vec![
            sqlx::query_as!(
                CommentRecord,
                r"SELECT c.id AS comment_id, c.answer_id, c.commented_by AS commented_by_id,
                u.name AS commented_by_name, u.role AS commented_by_role, c.content,
                c.timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>`
            FROM form_answer_comments c
            INNER JOIN users u ON u.id = c.commented_by
            WHERE c.id = ? AND c.answer_id = ? FOR UPDATE",
                comment_id.to_string(),
                answer_id_text,
            )
            .fetch_optional(&mut **transaction)
            .await
            .map_err(InfraError::from)?
            .ok_or(errors::domain::DomainError::NotFound)?
            .try_into()?,
        ],
        None => Vec::new(),
    };
    let thread = unsafe {
        CommentThread::from_raw_parts(
            answer_id,
            AnswerPublication::try_from(answer.publication)?,
            AnswerSettings::default()
                .change_visibility(answer_visibility)
                .change_answer_groups(AllowedUserGroups::new(answer_group_ids)),
            comments,
        )
    };
    AuthorizationGuard::<_, Update>::from(thread)
        .try_update(actor)
        .map_err(Into::into)
}

async fn insert_created_comment(
    transaction: &mut DatabaseTransaction,
    comment: &Comment,
    actor: &Actor,
) -> Result<(), Error> {
    let actor = account_user(actor)?;
    let comment_id = comment.comment_id().to_string();
    let answer_id = comment.answer_id().to_string();
    let commented_by = comment.commented_by().to_string();
    let content = comment.content().to_string();
    let timestamp = *comment.timestamp();
    sqlx::query!(
        "INSERT INTO form_answer_comments (id, answer_id, commented_by, content, timestamp) VALUES (?, ?, ?, ?, ?)",
        comment_id, answer_id, commented_by, content, timestamp,
    ).execute(&mut **transaction).await.map_err(InfraError::from)?;
    // The candidate author is the current actor; its role/name snapshot is intentionally captured now.
    sqlx::query!(
        r"INSERT INTO form_answer_comment_history
        (id, answer_id, comment_id, original_author_id, original_author_name, original_author_role,
         original_timestamp, action, content, operated_by_id, operated_by_name, operated_by_role, operated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, 'CREATE', ?, ?, ?, ?, ?)",
        Uuid::now_v7().to_string(), comment.answer_id().to_string(), comment.comment_id().to_string(),
        comment.commented_by().to_string(), actor.name(), actor.role().to_string(), timestamp,
        comment.content().to_string(), actor.id().to_string(), actor.name(), actor.role().to_string(), timestamp,
    ).execute(&mut **transaction).await.map_err(InfraError::from)?;
    Ok(())
}

async fn update_comment_with_history(
    transaction: &mut DatabaseTransaction,
    comment: &Comment,
    actor: &Actor,
    operated_at: DateTime<Utc>,
) -> Result<(), Error> {
    let actor = account_user(actor)?;
    let comment_id = comment.comment_id().to_string();
    let answer_id = comment.answer_id().to_string();
    let content = comment.content().to_string();
    let current = sqlx::query!(
        r"SELECT c.commented_by AS original_author_id, u.name AS original_author_name,
        u.role AS original_author_role, c.timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>`
        FROM form_answer_comments c INNER JOIN users u ON u.id = c.commented_by WHERE c.id = ?",
        comment_id,
    )
    .fetch_one(&mut **transaction)
    .await
    .map_err(InfraError::from)?;
    sqlx::query!(
        r"INSERT INTO form_answer_comment_history
        (id, answer_id, comment_id, original_author_id, original_author_name, original_author_role,
         original_timestamp, action, content, operated_by_id, operated_by_name, operated_by_role, operated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, 'UPDATE', ?, ?, ?, ?, ?)",
        Uuid::now_v7().to_string(), answer_id, comment_id, current.original_author_id,
        current.original_author_name, current.original_author_role, current.timestamp, content,
        actor.id().to_string(), actor.name(), actor.role().to_string(), operated_at,
    ).execute(&mut **transaction).await.map_err(InfraError::from)?;
    sqlx::query!(
        "UPDATE form_answer_comments SET content = ? WHERE id = ?",
        content,
        comment_id
    )
    .execute(&mut **transaction)
    .await
    .map_err(InfraError::from)?;
    Ok(())
}

async fn delete_comment_with_history(
    transaction: &mut DatabaseTransaction,
    deleted: &DeletedComment,
) -> Result<(), Error> {
    let comment = deleted.comment();
    let comment_id = comment.comment_id().to_string();
    let actor = deleted.deleted_by();
    let current = sqlx::query!(
        r"SELECT c.answer_id, c.commented_by AS original_author_id, u.name AS original_author_name,
        u.role AS original_author_role, c.content, c.timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>`
        FROM form_answer_comments c INNER JOIN users u ON u.id = c.commented_by WHERE c.id = ?",
        comment_id,
    ).fetch_one(&mut **transaction).await.map_err(InfraError::from)?;
    sqlx::query!(
        r"INSERT INTO form_answer_comment_history
        (id, answer_id, comment_id, original_author_id, original_author_name, original_author_role,
         original_timestamp, action, content, operated_by_id, operated_by_name, operated_by_role, operated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, 'DELETE', ?, ?, ?, ?, ?)",
        Uuid::now_v7().to_string(), current.answer_id, comment_id, current.original_author_id,
        current.original_author_name, current.original_author_role, current.timestamp, current.content,
        actor.id().to_string(), actor.name(), actor.role().to_string(), deleted.deleted_at(),
    ).execute(&mut **transaction).await.map_err(InfraError::from)?;
    sqlx::query!("DELETE FROM form_answer_comments WHERE id = ?", comment_id)
        .execute(&mut **transaction)
        .await
        .map_err(InfraError::from)?;
    Ok(())
}

fn account_user(actor: &Actor) -> Result<&domain::account::models::AccountUser, Error> {
    match actor {
        Actor::AccountUser(user) => Ok(user),
        _ => Err(errors::domain::DomainError::Forbidden.into()),
    }
}
