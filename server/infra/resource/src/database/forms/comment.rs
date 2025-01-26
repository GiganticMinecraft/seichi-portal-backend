use std::str::FromStr;

use async_trait::async_trait;
use domain::{
    form::{
        answer::models::AnswerId,
        comment::models::{Comment, CommentId},
    },
    user::models::Role,
};
use errors::infra::InfraError;

use crate::{
    database::{
        components::FormCommentDatabase,
        connection::{execute_and_values, query_all_and_values, ConnectionPool},
    },
    dto::{CommentDto, UserDto},
};

#[async_trait]
impl FormCommentDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn get_comment(&self, comment_id: CommentId) -> Result<Option<CommentDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let comment = query_all_and_values(
                    r"SELECT form_answer_comments.id AS content_id, answer_id, commented_by, name, role, content, timestamp FROM form_answer_comments
                    INNER JOIN users ON form_answer_comments.commented_by = users.id
                    WHERE form_answer_comments.id = ?",
                    [comment_id.into_inner().into()],
                    txn,
                ).await?;

                comment.into_iter().next().map(|rs| {
                    Ok::<_, InfraError>(CommentDto {
                        answer_id: rs.try_get("", "answer_id")?,
                        comment_id: rs.try_get("", "id")?,
                        content: rs.try_get("", "content")?,
                        timestamp: rs.try_get("", "time_stamp")?,
                        commented_by: UserDto {
                            name: rs.try_get("", "name")?,
                            id: rs.try_get("", "commented_by")?,
                            role: Role::from_str(rs.try_get::<String>("", "role")?.as_str())?,
                        },
                    })
                }).transpose()
            })
        })
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn get_comments(&self, answer_id: AnswerId) -> Result<Vec<CommentDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let comments = query_all_and_values(
                    r"SELECT form_answer_comments.id AS content_id, answer_id, commented_by, name, role, content, timestamp FROM form_answer_comments
                    INNER JOIN users ON form_answer_comments.commented_by = users.id
                    WHERE answer_id = ?",
                    [answer_id.into_inner().into()],
                    txn,
                ).await?;

                comments.into_iter().map(|rs| {
                    Ok::<_, InfraError>(CommentDto {
                        answer_id: rs.try_get("", "answer_id")?,
                        comment_id: rs.try_get("", "id")?,
                        content: rs.try_get("", "content")?,
                        timestamp: rs.try_get("", "time_stamp")?,
                        commented_by: UserDto {
                            name: rs.try_get("", "name")?,
                            id: rs.try_get("", "commented_by")?,
                            role: Role::from_str(rs.try_get::<String>("", "role")?.as_str())?,
                        },
                    })
                }).collect::<Result<Vec<_>, _>>()
            })
        })
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn post_comment(&self, answer_id: AnswerId, comment: &Comment) -> Result<(), InfraError> {
        let params = [
            comment.comment_id().into_inner().to_string().into(),
            answer_id.into_inner().into(),
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
                        VALUES (?, ?, ?, ?)",
                    params,
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    #[tracing::instrument]
    async fn delete_comment(&self, comment_id: CommentId) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "DELETE FROM form_answer_comments WHERE id = ?",
                    [comment_id.into_inner().into()],
                    txn,
                )
                .await?;

                Ok::<_, InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }
}
