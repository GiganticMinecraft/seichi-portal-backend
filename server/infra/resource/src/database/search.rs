use async_trait::async_trait;
use domain::{
    form::{
        answer::models::{AnswerLabel, FormAnswerContent},
        comment::models::Comment,
        models::{Form, FormLabel},
    },
    user::models::User,
};
use errors::infra::InfraError;
use itertools::Itertools;
use meilisearch_sdk::search::Selectors;

use crate::database::{components::SearchDatabase, connection::ConnectionPool};

#[async_trait]
impl SearchDatabase for ConnectionPool {
    async fn search_users(&self, query: &str) -> Result<Vec<User>, InfraError> {
        Ok(self
            .meilisearch_client
            .index("users")
            .search()
            .with_query(query)
            .with_attributes_to_highlight(Selectors::All)
            .execute::<User>()
            .await?
            .hits
            .into_iter()
            .map(|hit| hit.result)
            .collect_vec())
    }

    async fn search_forms(&self, query: &str) -> Result<Vec<Form>, InfraError> {
        Ok(self
            .meilisearch_client
            .index("form_meta_data")
            .search()
            .with_query(query)
            .with_attributes_to_highlight(Selectors::All)
            .execute::<Form>()
            .await?
            .hits
            .into_iter()
            .map(|hit| hit.result)
            .collect_vec())
    }

    async fn search_labels_for_forms(&self, query: &str) -> Result<Vec<FormLabel>, InfraError> {
        Ok(self
            .meilisearch_client
            .index("label_for_forms")
            .search()
            .with_query(query)
            .with_attributes_to_highlight(Selectors::All)
            .execute::<FormLabel>()
            .await?
            .hits
            .into_iter()
            .map(|hit| hit.result)
            .collect_vec())
    }

    async fn search_labels_for_answers(&self, query: &str) -> Result<Vec<AnswerLabel>, InfraError> {
        Ok(self
            .meilisearch_client
            .index("label_for_form_answers")
            .search()
            .with_query(query)
            .with_attributes_to_highlight(Selectors::All)
            .execute::<AnswerLabel>()
            .await?
            .hits
            .into_iter()
            .map(|hit| hit.result)
            .collect_vec())
    }

    async fn search_answers(&self, query: &str) -> Result<Vec<FormAnswerContent>, InfraError> {
        Ok(self
            .meilisearch_client
            .index("real_answers")
            .search()
            .with_query(query)
            .with_attributes_to_highlight(Selectors::All)
            .execute::<FormAnswerContent>()
            .await?
            .hits
            .into_iter()
            .map(|hit| hit.result)
            .collect_vec())
    }

    async fn search_comments(&self, query: &str) -> Result<Vec<Comment>, InfraError> {
        Ok(self
            .meilisearch_client
            .index("form_answer_comments")
            .search()
            .with_query(query)
            .with_attributes_to_highlight(Selectors::All)
            .execute::<Comment>()
            .await?
            .hits
            .into_iter()
            .map(|hit| hit.result)
            .collect_vec())
    }
}
