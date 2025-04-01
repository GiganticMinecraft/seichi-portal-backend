use crate::database::config::{MEILISEARCH, MeiliSearch};
use crate::database::meilisearch_schemas::MeilisearchStatsSchema;
use crate::database::{components::SearchDatabase, connection::ConnectionPool};
use async_trait::async_trait;
use domain::search::models::NumberOfRecordsPerAggregate;
use domain::{
    form::{
        answer::models::{AnswerLabel, FormAnswerContent},
        comment::models::Comment,
        models::{Form, FormLabel},
    },
    search::models::{Operation, SearchableFields, SearchableFieldsWithOperation},
    user::models::User,
};
use errors::infra::InfraError;
use itertools::Itertools;
use meilisearch_sdk::search::Selectors;

#[async_trait]
impl SearchDatabase for ConnectionPool {
    #[tracing::instrument]
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

    #[tracing::instrument]
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

    #[tracing::instrument]
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

    #[tracing::instrument]
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

    #[tracing::instrument]
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

    #[tracing::instrument]
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

    #[tracing::instrument]
    async fn sync_search_engine(
        &self,
        data: &[SearchableFieldsWithOperation],
    ) -> Result<(), InfraError> {
        let futures = data
            .iter()
            .map(async |(searchable_fields, operation)| match operation {
                Operation::Create | Operation::Update => match searchable_fields {
                    SearchableFields::FormMetaData(data) => {
                        self.meilisearch_client
                            .index("form_meta_data")
                            .add_or_replace(&[data], Some("id"))
                            .await
                    }
                    SearchableFields::RealAnswers(answers) => {
                        self.meilisearch_client
                            .index("real_answers")
                            .add_or_replace(&[answers], Some("id"))
                            .await
                    }
                    SearchableFields::FormAnswerComments(comments) => {
                        self.meilisearch_client
                            .index("form_answer_comments")
                            .add_or_replace(&[comments], Some("id"))
                            .await
                    }
                    SearchableFields::LabelForFormAnswers(label) => {
                        self.meilisearch_client
                            .index("label_for_form_answers")
                            .add_or_replace(&[label], Some("id"))
                            .await
                    }
                    SearchableFields::LabelForForms(label) => {
                        self.meilisearch_client
                            .index("label_for_forms")
                            .add_or_replace(&[label], Some("id"))
                            .await
                    }
                    SearchableFields::Users(users) => {
                        self.meilisearch_client
                            .index("users")
                            .add_or_replace(&[users], Some("id"))
                            .await
                    }
                },
                Operation::Delete => match searchable_fields {
                    SearchableFields::FormMetaData(data) => {
                        self.meilisearch_client
                            .index("form_meta_data")
                            .delete_document(data.id.into_inner().to_string())
                            .await
                    }
                    SearchableFields::RealAnswers(answers) => {
                        self.meilisearch_client
                            .index("real_answers")
                            .delete_document(answers.id.to_string())
                            .await
                    }
                    SearchableFields::FormAnswerComments(comments) => {
                        self.meilisearch_client
                            .index("form_answer_comments")
                            .delete_document(comments.id.into_inner().to_string())
                            .await
                    }
                    SearchableFields::LabelForFormAnswers(label) => {
                        self.meilisearch_client
                            .index("label_for_form_answers")
                            .delete_document(label.id.into_inner().to_string())
                            .await
                    }
                    SearchableFields::LabelForForms(label) => {
                        self.meilisearch_client
                            .index("label_for_forms")
                            .delete_document(label.id.into_inner().to_string())
                            .await
                    }
                    SearchableFields::Users(users) => {
                        self.meilisearch_client
                            .index("users")
                            .delete_document(users.id.to_string())
                            .await
                    }
                },
            })
            .collect::<Vec<_>>();

        futures::future::try_join_all(futures).await?;

        Ok(())
    }

    #[tracing::instrument]
    async fn search_engine_stats(&self) -> Result<NumberOfRecordsPerAggregate, InfraError> {
        let client = reqwest::Client::new();

        let MeiliSearch { host, api_key } = &*MEILISEARCH;

        let mut request = client.get(format!("{}/stats", host));

        if let Some(api_key) = api_key {
            request = request.header("X-Meili-API-Key", api_key);
        }

        let response = request.send().await?;

        Ok(
            serde_json::from_str::<MeilisearchStatsSchema>(response.text().await?.as_str())?
                .indexes
                .into(),
        )
    }
}
