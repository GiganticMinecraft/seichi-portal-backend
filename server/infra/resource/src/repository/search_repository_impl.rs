use crate::{
    database::components::{DatabaseComponents, FormAnswerDatabase, SearchDatabase},
    repository::Repository,
};
use async_trait::async_trait;
use domain::form::answer::models::AnswerEntry;
use domain::search::models::NumberOfRecordsPerAggregate;
use domain::{
    form::{
        answer::models::AnswerLabel,
        comment::models::Comment,
        models::{ActiveForm, FormLabel},
    },
    repository::search_repository::SearchRepository,
    search::models::SearchableFieldsWithOperation,
    types::authorization_guard::{AuthorizationGuard, Read},
    user::models::ActiveUser,
};
use errors::Error;
use itertools::Itertools;

#[async_trait]
impl<Client: DatabaseComponents + 'static> SearchRepository for Repository<Client> {
    async fn search_users(
        &self,
        query: &str,
    ) -> Result<Vec<AuthorizationGuard<ActiveUser, Read>>, Error> {
        Ok(self
            .client
            .search()
            .search_users(query)
            .await?
            .into_iter()
            .map(Into::into)
            .collect_vec())
    }

    async fn search_forms(
        &self,
        query: &str,
    ) -> Result<Vec<AuthorizationGuard<ActiveForm, Read>>, Error> {
        Ok(self
            .client
            .search()
            .search_forms(query)
            .await?
            .into_iter()
            .map(Into::into)
            .collect_vec())
    }

    async fn search_labels_for_forms(
        &self,
        query: &str,
    ) -> Result<Vec<AuthorizationGuard<FormLabel, Read>>, Error> {
        Ok(self
            .client
            .search()
            .search_labels_for_forms(query)
            .await?
            .into_iter()
            .map(Into::into)
            .collect_vec())
    }

    async fn search_labels_for_answers(
        &self,
        query: &str,
    ) -> Result<Vec<AuthorizationGuard<AnswerLabel, Read>>, Error> {
        Ok(self
            .client
            .search()
            .search_labels_for_answers(query)
            .await?
            .into_iter()
            .map(Into::into)
            .collect_vec())
    }

    async fn search_answers(&self, query: &str) -> Result<Vec<AnswerEntry>, Error> {
        let real_answers = self.client.search().search_answers(query).await?;
        let answer_ids = real_answers
            .iter()
            .map(|answer| answer.answer_id)
            .collect_vec();

        self.client
            .form_answer()
            .get_answers_by_answer_ids(answer_ids)
            .await?
            .into_iter()
            .map(TryInto::<AnswerEntry>::try_into)
            .collect()
    }

    async fn search_comments(&self, query: &str) -> Result<Vec<Comment>, Error> {
        Ok(self
            .client
            .search()
            .search_comments(query)
            .await?
            .into_iter()
            .map(Ok::<_, Error>)
            .collect::<Result<Vec<_>, _>>()?)
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

    async fn initialize_search_engine(&self) -> Result<(), Error> {
        self.client
            .search()
            .initialize_search_engine()
            .await
            .map_err(Into::into)
    }
}
