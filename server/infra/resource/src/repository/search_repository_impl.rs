use crate::{
    database::components::{DatabaseComponents, SearchDatabase},
    repository::Repository,
};
use async_trait::async_trait;
use domain::{
    form::models::FormId,
    repository::search_repository::SearchRepository,
    search::models::{
        AnswerLabelSearchHit, AnswerSearchHit, CommentSearchHit, FormLabelSearchHit, FormSearchHit,
        NumberOfRecordsPerAggregate, SearchableFieldsWithOperation, UserSearchHit,
    },
};
use errors::Error;

#[async_trait]
impl<Client: DatabaseComponents + 'static> SearchRepository for Repository<Client> {
    async fn search_users(&self, query: &str) -> Result<Vec<UserSearchHit>, Error> {
        self.client
            .search()
            .search_users(query)
            .await
            .map_err(Into::into)
    }

    async fn search_forms(&self, query: &str) -> Result<Vec<FormSearchHit>, Error> {
        self.client
            .search()
            .search_forms(query)
            .await
            .map_err(Into::into)
    }

    async fn search_labels_for_forms(&self, query: &str) -> Result<Vec<FormLabelSearchHit>, Error> {
        self.client
            .search()
            .search_labels_for_forms(query)
            .await
            .map_err(Into::into)
    }

    async fn search_labels_for_answers(
        &self,
        query: &str,
    ) -> Result<Vec<AnswerLabelSearchHit>, Error> {
        self.client
            .search()
            .search_labels_for_answers(query)
            .await
            .map_err(Into::into)
    }

    async fn search_answers(
        &self,
        query: &str,
        form_id: Option<FormId>,
    ) -> Result<Vec<AnswerSearchHit>, Error> {
        self.client
            .search()
            .search_answers(query, form_id)
            .await
            .map_err(Into::into)
    }

    async fn search_comments(&self, query: &str) -> Result<Vec<CommentSearchHit>, Error> {
        self.client
            .search()
            .search_comments(query)
            .await
            .map_err(Into::into)
    }

    async fn sync_search_engine(
        &self,
        data: &[SearchableFieldsWithOperation],
    ) -> Result<(), Error> {
        self.client
            .search()
            .sync_search_engine(data)
            .await
            .map_err(Into::into)
    }

    async fn fetch_search_engine_stats(&self) -> Result<NumberOfRecordsPerAggregate, Error> {
        self.client
            .search()
            .search_engine_stats()
            .await
            .map_err(Into::into)
    }

    async fn initialize_search_engine(&self) -> Result<bool, Error> {
        self.client
            .search()
            .initialize_search_engine()
            .await
            .map_err(Into::into)
    }
}
