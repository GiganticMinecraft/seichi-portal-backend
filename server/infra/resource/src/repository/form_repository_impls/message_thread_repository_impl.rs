use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    account::models::{AccountUser, UserSnapshot},
    form::{
        answer::AnswerId,
        message::{
            DeletedMessage, Message, MessageBody, MessageHistoryAction, MessageHistoryEntry,
            MessageHistoryPagePosition, MessagePost,
        },
        message_thread::MessageThread,
    },
    pagination::{Page, PageRequest},
    repository::form::message_thread_repository::MessageThreadRepository,
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Delete, Read, Update},
};
use errors::{Error, infra::InfraError};
use std::str::FromStr;
use uuid::Uuid;

use crate::{
    database::{
        components::{DatabaseComponents, FormMessageDatabase, FormMessageThreadDatabase},
        connection::DatabaseTransaction,
    },
    repository::Repository,
};

#[async_trait]
impl<Client> MessageThreadRepository for Repository<Client>
where
    Client: DatabaseComponents<TransactionAcrossComponents = DatabaseTransaction> + 'static,
{
    #[tracing::instrument(skip(self))]
    async fn create(&self, message_thread: Allowed<MessageThread, Create>) -> Result<(), Error> {
        let operated_by = account_user_snapshot(message_thread.actor())?;
        let thread = message_thread.into_inner();

        self.client
            .form_message_thread()
            .create_message_thread(&thread, &operated_by)
            .await?;

        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn get_by_answer_id(
        &self,
        answer_id: AnswerId,
    ) -> Result<Option<AuthorizationGuard<MessageThread, Read>>, Error> {
        let answer_id_str = answer_id.into_inner().to_string();

        let Some(answer_author_id_str) = self
            .client
            .form_message_thread()
            .get_thread_author_by_answer_id(&answer_id_str)
            .await?
        else {
            return Ok(None);
        };

        let messages = self
            .client
            .form_message()
            .fetch_messages_by_answer_id(answer_id)
            .await?
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, _>>()?;

        let thread = unsafe {
            MessageThread::from_raw_parts(
                answer_id,
                Uuid::from_str(&answer_author_id_str)
                    .map_err(InfraError::from)?
                    .into(),
                messages,
            )
        };

        Ok(Some(AuthorizationGuard::<MessageThread, Read>::from(
            thread,
        )))
    }

    #[tracing::instrument(skip(self))]
    async fn append(&self, post: Allowed<MessagePost, Create>) -> Result<(), Error> {
        let operated_by = account_user_snapshot(post.actor())?;
        let post = post.into_inner();
        let answer_id = *post.answer_id();
        let message = post.into_message();
        self.client
            .form_message()
            .post_message(&message, answer_id, &operated_by)
            .await?;
        Ok(())
    }

    async fn update_message(
        &self,
        message: Allowed<Message, Update>,
        updated_at: DateTime<Utc>,
    ) -> Result<(), Error> {
        let operated_by = account_user(message.actor())?;
        self.client
            .form_message()
            .update_message_with_history(message.value(), operated_by, updated_at)
            .await?;
        Ok(())
    }

    async fn delete_message(&self, message: Allowed<DeletedMessage, Delete>) -> Result<(), Error> {
        self.client
            .form_message()
            .delete_message_with_history(message.value())
            .await?;
        Ok(())
    }

    async fn history(
        &self,
        message_thread: &Allowed<MessageThread, Read>,
        request: PageRequest<MessageHistoryPagePosition>,
    ) -> Result<Page<Allowed<MessageHistoryEntry, Read>, MessageHistoryPagePosition>, Error> {
        let page = self
            .client
            .form_message()
            .fetch_history(
                *message_thread.answer_id(),
                request,
                message_thread.can_read_deleted_message_history(),
            )
            .await?;
        let (records, next) = page.into_parts();
        let items = records
            .into_iter()
            .map(|record| {
                let action = message_history_action(record.action.as_str())?;
                let history_entry = unsafe {
                    MessageHistoryEntry::from_raw_parts(
                        Uuid::parse_str(&record.id)
                            .map_err(InfraError::from)?
                            .into(),
                        Uuid::parse_str(&record.answer_id)
                            .map_err(InfraError::from)?
                            .into(),
                        Uuid::parse_str(&record.message_id)
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
                        MessageBody::new(record.body.try_into()?),
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
                message_thread
                    .authorize_message_history_entry(history_entry)
                    .map_err(Error::from)
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(Page::new(items, next))
    }
}

fn message_history_action(action: &str) -> Result<MessageHistoryAction, InfraError> {
    match action {
        "CREATE" => Ok(MessageHistoryAction::Create),
        "UPDATE" => Ok(MessageHistoryAction::Update),
        "DELETE" => Ok(MessageHistoryAction::Delete),
        action => Err(InfraError::Unexpected {
            cause: format!("invalid message history payload for action: {action}"),
        }),
    }
}

fn account_user(actor: &domain::auth::Actor) -> Result<&AccountUser, Error> {
    match actor {
        domain::auth::Actor::AccountUser(user) => Ok(user),
        _ => Err(InfraError::Unexpected {
            cause: "message operation actor is not an account user".to_string(),
        }
        .into()),
    }
}

fn account_user_snapshot(actor: &domain::auth::Actor) -> Result<UserSnapshot, Error> {
    account_user(actor).map(UserSnapshot::from)
}
