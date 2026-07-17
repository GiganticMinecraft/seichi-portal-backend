use async_trait::async_trait;
use chrono::{DateTime, Utc};
use errors::Error;
use mockall::automock;

use crate::{
    form::{
        answer::AnswerId,
        message::{
            DeletedMessage, Message, MessageHistoryEntry, MessageHistoryPagePosition, MessagePost,
        },
        message_thread::MessageThread,
    },
    pagination::{Page, PageRequest},
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Delete, Read, Update},
};

#[automock]
#[async_trait]
pub trait MessageThreadRepository: Send + Sync + 'static {
    async fn create(&self, message_thread: Allowed<MessageThread, Create>) -> Result<(), Error>;
    async fn get_by_answer_id(
        &self,
        answer_id: AnswerId,
    ) -> Result<Option<AuthorizationGuard<MessageThread, Read>>, Error>;
    async fn append(&self, post: Allowed<MessagePost, Create>) -> Result<(), Error>;
    async fn update_message(
        &self,
        message: Allowed<Message, Update>,
        updated_at: DateTime<Utc>,
    ) -> Result<(), Error>;
    async fn delete_message(&self, message: Allowed<DeletedMessage, Delete>) -> Result<(), Error>;
    async fn history(
        &self,
        message_thread: &Allowed<MessageThread, Read>,
        request: PageRequest<MessageHistoryPagePosition>,
    ) -> Result<Page<Allowed<MessageHistoryEntry, Read>, MessageHistoryPagePosition>, Error>;
}
