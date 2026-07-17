use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    account::models::UserSnapshot,
    form::{
        answer::AnswerEntry,
        comment::{
            Comment, CommentContent, CommentHistoryAction, CommentHistoryEntry,
            CommentHistoryPagePosition, DeletedComment,
        },
    },
    pagination::{Page, PageRequest},
    repository::form::comment_repository::CommentRepository,
    types::authorization_guard::{Allowed, Create, Delete, Read, Update},
};
use errors::{Error, infra::InfraError};
use std::str::FromStr;
use uuid::Uuid;

use crate::{
    database::{
        components::{DatabaseComponents, FormCommentDatabase},
        connection::DatabaseTransaction,
    },
    repository::Repository,
};

#[async_trait]
impl<Client> CommentRepository for Repository<Client>
where
    Client: DatabaseComponents<TransactionAcrossComponents = DatabaseTransaction> + 'static,
{
    #[tracing::instrument(skip(self, comment))]
    async fn create(&self, comment: Allowed<Comment, Create>) -> Result<(), Error> {
        let operated_by = account_user_snapshot(comment.actor())?;
        let comment = comment.into_inner();

        self.client
            .form_comment()
            .create_comment(&comment, &operated_by)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self, answer))]
    async fn find_by_answer(
        &self,
        answer: &Allowed<AnswerEntry, Read>,
    ) -> Result<Vec<Allowed<Comment, Read>>, Error> {
        self.client
            .form_comment()
            .get_comments(*answer.value().id())
            .await?
            .into_iter()
            .map(|record| {
                let comment = TryInto::<Comment>::try_into(record)?;
                answer.authorize_comment(comment).map_err(Error::from)
            })
            .collect()
    }

    #[tracing::instrument(skip(self, comment))]
    async fn update(
        &self,
        comment: Allowed<Comment, Update>,
        updated_at: DateTime<Utc>,
    ) -> Result<(), Error> {
        let operated_by = account_user(comment.actor())?;

        self.client
            .form_comment()
            .update_comment_with_history(comment.value(), operated_by, updated_at)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self, comment))]
    async fn delete(&self, comment: Allowed<DeletedComment, Delete>) -> Result<(), Error> {
        self.client
            .form_comment()
            .delete_comment_with_history(comment.value())
            .await?;
        Ok(())
    }

    async fn history(
        &self,
        answer: &Allowed<AnswerEntry, Read>,
        request: PageRequest<CommentHistoryPagePosition>,
    ) -> Result<Page<Allowed<CommentHistoryEntry, Read>, CommentHistoryPagePosition>, Error> {
        let page = self
            .client
            .form_comment()
            .get_history(
                *answer.value().id(),
                request,
                answer.can_read_deleted_comment_history(),
            )
            .await?;
        let (records, next) = page.into_parts();
        let items = records
            .into_iter()
            .map(|record| {
                let action = comment_history_action(record.action.as_str())?;
                let history_entry = unsafe {
                    CommentHistoryEntry::from_raw_parts(
                        Uuid::parse_str(&record.id)
                            .map_err(InfraError::from)?
                            .into(),
                        Uuid::parse_str(&record.answer_id)
                            .map_err(InfraError::from)?
                            .into(),
                        Uuid::parse_str(&record.comment_id)
                            .map_err(InfraError::from)?
                            .into(),
                        UserSnapshot::new(
                            Uuid::parse_str(&record.original_author_id)
                                .map_err(InfraError::from)?
                                .into(),
                            record.original_author_name,
                            domain::account::models::Role::from_str(&record.original_author_role)
                                .map_err(InfraError::from)?,
                        ),
                        record.original_timestamp,
                        action,
                        CommentContent::new(record.content.try_into()?),
                        UserSnapshot::new(
                            Uuid::parse_str(&record.operated_by_id)
                                .map_err(InfraError::from)?
                                .into(),
                            record.operated_by_name,
                            domain::account::models::Role::from_str(&record.operated_by_role)
                                .map_err(InfraError::from)?,
                        ),
                        record.operated_at,
                    )
                };
                answer
                    .authorize_comment_history_entry(history_entry)
                    .map_err(Error::from)
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(Page::new(items, next))
    }

    #[tracing::instrument(skip(self))]
    async fn size(&self) -> Result<u32, Error> {
        self.client.form_comment().size().await.map_err(Into::into)
    }
}

fn comment_history_action(action: &str) -> Result<CommentHistoryAction, InfraError> {
    match action {
        "CREATE" => Ok(CommentHistoryAction::Create),
        "UPDATE" => Ok(CommentHistoryAction::Update),
        "DELETE" => Ok(CommentHistoryAction::Delete),
        action => Err(InfraError::Unexpected {
            cause: format!("invalid comment history payload for action: {action}"),
        }),
    }
}

fn account_user(
    actor: &domain::auth::Actor,
) -> Result<&domain::account::models::AccountUser, Error> {
    match actor {
        domain::auth::Actor::AccountUser(user) => Ok(user),
        _ => Err(InfraError::Unexpected {
            cause: "comment operation actor is not an account user".to_string(),
        }
        .into()),
    }
}

fn account_user_snapshot(actor: &domain::auth::Actor) -> Result<UserSnapshot, Error> {
    account_user(actor).map(UserSnapshot::from)
}
