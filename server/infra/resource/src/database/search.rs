use std::sync::Arc;

use async_trait::async_trait;
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
use tokio::sync::{Notify, mpsc::Receiver};

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

    async fn start_sync(
        &self,
        receiver: Receiver<SearchableFieldsWithOperation>,
        shutdown_notifier: Arc<Notify>,
    ) -> Result<(), InfraError> {
        let self_clone = self.clone();
        let mut receiver = receiver;

        tokio::spawn({
            async move {
                loop {
                    tokio::select! {
                        _ = shutdown_notifier.notified() => {
                            break;
                        },
                        _ = async {
                            if let Some((searchable_fields, operation)) = receiver.recv().await {
                                match operation {
                                    Operation::Create | Operation::Update => {
                                        match searchable_fields {
                                            SearchableFields::FormMetaData(data) => {
                                                self_clone.meilisearch_client
                                                    .index("form_meta_data")
                                                    .add_or_replace(&[data], Some("id"))
                                                    .await?
                                            },
                                            SearchableFields::RealAnswers(answers) => {
                                                self_clone.meilisearch_client
                                                    .index("real_answers")
                                                    .add_or_replace(&[answers], Some("id"))
                                                    .await?
                                            },
                                            SearchableFields::FormAnswerComments(comments) => {
                                                self_clone.meilisearch_client
                                                    .index("form_answer_comments")
                                                    .add_or_replace(&[comments], Some("id"))
                                                    .await?
                                            },
                                            SearchableFields::LabelForFormAnswers(label) => {
                                                self_clone.meilisearch_client
                                                    .index("label_for_form_answers")
                                                    .add_or_replace(&[label], Some("id"))
                                                    .await?
                                            },
                                            SearchableFields::LabelForForms(label) => {
                                                self_clone.meilisearch_client
                                                    .index("label_for_forms")
                                                    .add_or_replace(&[label], Some("id"))
                                                    .await?
                                            },
                                            SearchableFields::Users(users) => {
                                                self_clone.meilisearch_client
                                                    .index("users")
                                                    .add_or_replace(&[users], Some("id"))
                                                    .await?
                                            }
                                        };
                                    },
                                    Operation::Delete => {
                                        match searchable_fields {
                                            SearchableFields::FormMetaData(data) => {
                                                self_clone.meilisearch_client
                                                    .index("form_meta_data")
                                                    .delete_document(data.id.into_inner().to_string())
                                                    .await?
                                            },
                                            SearchableFields::RealAnswers(answers) => {
                                                self_clone.meilisearch_client
                                                    .index("real_answers")
                                                    .delete_document(answers.id.to_string())
                                                    .await?
                                            },
                                            SearchableFields::FormAnswerComments(comments) => {
                                                self_clone.meilisearch_client
                                                    .index("form_answer_comments")
                                                    .delete_document(comments.id.into_inner().to_string())
                                                    .await?
                                            },
                                            SearchableFields::LabelForFormAnswers(label) => {
                                                self_clone.meilisearch_client
                                                    .index("label_for_form_answers")
                                                    .delete_document(label.id.into_inner().to_string())
                                                    .await?
                                            },
                                            SearchableFields::LabelForForms(label) => {
                                                self_clone.meilisearch_client
                                                    .index("label_for_forms")
                                                    .delete_document(label.id.into_inner().to_string())
                                                    .await?
                                            },
                                            SearchableFields::Users(users) => {
                                                self_clone.meilisearch_client
                                                    .index("users")
                                                    .delete_document(users.id.to_string())
                                                    .await?
                                            }
                                        };
                                    }
                                }
                            }

                            Ok::<_, InfraError>(())
                        } => {}
                    }
                }
            }
        });

        Ok(())
    }
}
