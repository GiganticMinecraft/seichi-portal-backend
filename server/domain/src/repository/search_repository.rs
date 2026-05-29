use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::search::models::{
    AnswerLabelSearchHit, AnswerSearchHit, CommentSearchHit, FormLabelSearchHit, FormSearchHit,
    NumberOfRecordsPerAggregate, SearchableFieldsWithOperation, UserSearchHit,
};

#[automock]
#[async_trait]
pub trait SearchRepository: Send + Sync + 'static {
    async fn search_users(&self, query: &str) -> Result<Vec<UserSearchHit>, Error>;
    async fn search_forms(&self, query: &str) -> Result<Vec<FormSearchHit>, Error>;
    async fn search_labels_for_forms(&self, query: &str) -> Result<Vec<FormLabelSearchHit>, Error>;
    async fn search_labels_for_answers(
        &self,
        query: &str,
    ) -> Result<Vec<AnswerLabelSearchHit>, Error>;
    async fn search_answers(&self, query: &str) -> Result<Vec<AnswerSearchHit>, Error>;
    async fn search_comments(&self, query: &str) -> Result<Vec<CommentSearchHit>, Error>;
    async fn sync_search_engine(&self, data: &[SearchableFieldsWithOperation])
    -> Result<(), Error>;
    async fn fetch_search_engine_stats(&self) -> Result<NumberOfRecordsPerAggregate, Error>;
    async fn initialize_search_engine(&self) -> Result<(), Error>;
}
