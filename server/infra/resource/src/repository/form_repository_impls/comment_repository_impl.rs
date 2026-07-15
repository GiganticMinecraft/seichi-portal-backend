use async_trait::async_trait;
use domain::{
    auth::Actor,
    form::{
        answer::AnswerEntry,
        comment::{
            Comment, CommentHistoryAction, CommentHistoryEntry, CommentHistoryPagePosition,
            HistoryUserSnapshot,
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
        let comment = comment.into_inner();

        self.client.form_comment().create_comment(&comment).await?;
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
    async fn update(&self, comment: Allowed<Comment, Update>) -> Result<(), Error> {
        let operated_by = account_user_actor(comment.actor())?;

        self.client
            .form_comment()
            .update_comment_with_history(comment.value(), operated_by)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self, comment))]
    async fn delete(&self, comment: Allowed<Comment, Delete>) -> Result<(), Error> {
        let operated_by = account_user_actor(comment.actor())?;

        self.client
            .form_comment()
            .delete_comment_with_history(*comment.value().comment_id(), operated_by)
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
            .get_history(*answer.value().id(), request)
            .await?;
        let (records, next) = page.into_parts();
        let items = records
            .into_iter()
            .map(|record| {
                let action = match record.action.as_str() {
                    "UPDATE" => CommentHistoryAction::Update,
                    "DELETE" => CommentHistoryAction::Delete,
                    action => {
                        return Err(InfraError::Unexpected {
                            cause: format!("unknown comment history action: {action}"),
                        }
                        .into());
                    }
                };
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
                        HistoryUserSnapshot::new(
                            Uuid::parse_str(&record.original_author_id)
                                .map_err(InfraError::from)?
                                .into(),
                            record.original_author_name,
                            domain::account::models::Role::from_str(&record.original_author_role)
                                .map_err(InfraError::from)?,
                        ),
                        record.original_timestamp,
                        action,
                        record.before_content,
                        record.after_content,
                        HistoryUserSnapshot::new(
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

fn account_user_actor(actor: &Actor) -> Result<&domain::account::models::AccountUser, Error> {
    match actor {
        Actor::AccountUser(user) => Ok(user),
        _ => Err(InfraError::Unexpected {
            cause: "comment operation actor is not an account user".to_string(),
        }
        .into()),
    }
}
