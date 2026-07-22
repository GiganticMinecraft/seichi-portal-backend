use crate::database::config::{MEILISEARCH, MeiliSearch};
use crate::database::meilisearch_schemas::MeilisearchStatsSchema;
use crate::database::{
    components::{FormAnswerDatabase, SearchDatabase},
    connection::ConnectionPool,
};
use async_trait::async_trait;
use domain::search::models::{
    AnswerLabelSearchHit, AnswerSearchHit, AnswerTitleSearchDocument, CommentSearchHit,
    FormAnswerComments, FormLabelSearchHit, FormMetaData, FormSearchHit, LabelForFormAnswers,
    LabelForForms, NumberOfRecordsPerAggregate, UserSearchHit, Users,
};
use domain::{
    account::models::UserId,
    search::models::{Operation, SearchableFields, SearchableFieldsWithOperation},
};
use errors::infra::InfraError;
use itertools::Itertools;
use meilisearch_sdk::search::Selectors;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Serialize, Deserialize)]
struct AnswerContentSearchDocument {
    id: domain::form::answer::FormAnswerContentId,
    form_id: domain::form::models::FormId,
    answer_id: domain::form::answer::AnswerId,
    question_id: domain::form::question::QuestionId,
    answer: String,
}

#[derive(Deserialize)]
struct FormIdPresence {
    #[serde(default)]
    form_id: Option<domain::form::models::FormId>,
}

fn form_filter(form_id: domain::form::models::FormId) -> String {
    format!("form_id = \"{form_id}\"")
}

async fn answer_documents_need_reprojection(
    connection: &ConnectionPool,
) -> Result<bool, InfraError> {
    for index in ["answers", "real_answers"] {
        let missing_form_id = connection
            .meilisearch_client
            .index(index)
            .search()
            .with_filter("form_id NOT EXISTS")
            .with_limit(1)
            .execute::<FormIdPresence>()
            .await?
            .hits
            .into_iter()
            .any(|hit| hit.result.form_id.is_none());
        if missing_form_id {
            return Ok(true);
        }
    }

    Ok(false)
}

fn merge_answer_hits(
    title_answer_ids: impl IntoIterator<Item = domain::form::answer::AnswerId>,
    content_answer_ids: impl IntoIterator<Item = domain::form::answer::AnswerId>,
) -> Vec<AnswerSearchHit> {
    title_answer_ids
        .into_iter()
        .chain(content_answer_ids)
        .map(|answer_id| AnswerSearchHit { answer_id })
        .unique_by(|hit| hit.answer_id)
        .collect()
}

#[async_trait]
impl SearchDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn search_users(&self, query: &str) -> Result<Vec<UserSearchHit>, InfraError> {
        Ok(self
            .meilisearch_client
            .index("users")
            .search()
            .with_query(query)
            .with_attributes_to_highlight(Selectors::All)
            .execute::<Users>()
            .await?
            .hits
            .into_iter()
            .map(|hit| UserSearchHit {
                user_id: UserId::from(hit.result.id),
            })
            .collect_vec())
    }

    #[tracing::instrument]
    async fn search_forms(&self, query: &str) -> Result<Vec<FormSearchHit>, InfraError> {
        Ok(self
            .meilisearch_client
            .index("form_meta_data")
            .search()
            .with_query(query)
            .with_attributes_to_highlight(Selectors::All)
            .execute::<FormMetaData>()
            .await?
            .hits
            .into_iter()
            .map(|hit| FormSearchHit {
                form_id: hit.result.id,
            })
            .collect_vec())
    }

    #[tracing::instrument]
    async fn search_labels_for_forms(
        &self,
        query: &str,
    ) -> Result<Vec<FormLabelSearchHit>, InfraError> {
        Ok(self
            .meilisearch_client
            .index("label_for_forms")
            .search()
            .with_query(query)
            .with_attributes_to_highlight(Selectors::All)
            .execute::<LabelForForms>()
            .await?
            .hits
            .into_iter()
            .map(|hit| FormLabelSearchHit {
                label_id: hit.result.id,
            })
            .collect_vec())
    }

    #[tracing::instrument]
    async fn search_labels_for_answers(
        &self,
        query: &str,
    ) -> Result<Vec<AnswerLabelSearchHit>, InfraError> {
        Ok(self
            .meilisearch_client
            .index("label_for_form_answers")
            .search()
            .with_query(query)
            .with_attributes_to_highlight(Selectors::All)
            .execute::<LabelForFormAnswers>()
            .await?
            .hits
            .into_iter()
            .map(|hit| AnswerLabelSearchHit {
                label_id: hit.result.id,
            })
            .collect_vec())
    }

    #[tracing::instrument]
    async fn search_answers(
        &self,
        query: &str,
        form_id: Option<domain::form::models::FormId>,
    ) -> Result<Vec<AnswerSearchHit>, InfraError> {
        let filter = form_id.map(form_filter);
        let title_search = async {
            let index = self.meilisearch_client.index("answers");
            let mut search = index.search();
            search
                .with_query(query)
                .with_attributes_to_highlight(Selectors::All);
            if let Some(filter) = filter.as_deref() {
                search.with_filter(filter);
            }
            search.execute::<AnswerTitleSearchDocument>().await
        };
        let content_search = async {
            let index = self.meilisearch_client.index("real_answers");
            let mut search = index.search();
            search
                .with_query(query)
                .with_attributes_to_highlight(Selectors::All);
            if let Some(filter) = filter.as_deref() {
                search.with_filter(filter);
            }
            search.execute::<AnswerContentSearchDocument>().await
        };
        let (title_results, content_results) = futures::try_join!(title_search, content_search)?;

        Ok(merge_answer_hits(
            title_results.hits.into_iter().map(|hit| hit.result.id),
            content_results
                .hits
                .into_iter()
                .map(|hit| hit.result.answer_id),
        ))
    }

    #[tracing::instrument]
    async fn search_comments(&self, query: &str) -> Result<Vec<CommentSearchHit>, InfraError> {
        Ok(self
            .meilisearch_client
            .index("form_answer_comments")
            .search()
            .with_query(query)
            .with_attributes_to_highlight(Selectors::All)
            .execute::<FormAnswerComments>()
            .await?
            .hits
            .into_iter()
            .map(|hit| CommentSearchHit {
                comment_id: hit.result.id,
                answer_id: hit.result.answer_id,
            })
            .collect_vec())
    }

    #[tracing::instrument]
    async fn sync_search_engine(
        &self,
        data: &[SearchableFieldsWithOperation],
    ) -> Result<(), InfraError> {
        let mut form_ids_by_answer_id = data
            .iter()
            .filter_map(|(fields, operation)| match (fields, operation) {
                (SearchableFields::AnswerTitle(answer), Operation::Create | Operation::Update) => {
                    Some((answer.id, answer.form_id))
                }
                _ => None,
            })
            .collect::<HashMap<_, _>>();
        let unresolved_content_answer_ids = data
            .iter()
            .filter_map(|(fields, operation)| match (fields, operation) {
                (SearchableFields::RealAnswers(content), Operation::Create | Operation::Update) => {
                    Some(content.answer_id)
                }
                _ => None,
            })
            .unique()
            .filter(|answer_id| !form_ids_by_answer_id.contains_key(answer_id))
            .collect_vec();
        let database_form_ids: HashMap<
            domain::form::answer::AnswerId,
            domain::form::models::FormId,
        > = if unresolved_content_answer_ids.is_empty() {
            HashMap::new()
        } else {
            self.get_answers_by_answer_ids(unresolved_content_answer_ids)
                .await?
                .into_iter()
                .map(|record| {
                    Ok((
                        Uuid::parse_str(&record.id)?.into(),
                        Uuid::parse_str(&record.form_id)?.into(),
                    ))
                })
                .collect::<Result<HashMap<_, _>, InfraError>>()?
        };
        form_ids_by_answer_id.extend(database_form_ids);
        let content_documents = data
            .iter()
            .filter_map(|(fields, operation)| match (fields, operation) {
                (SearchableFields::RealAnswers(content), Operation::Create | Operation::Update) => {
                    Some(content)
                }
                _ => None,
            })
            .map(|content| {
                let form_id = form_ids_by_answer_id.get(&content.answer_id).copied().ok_or_else(|| {
                    InfraError::Unexpected {
                        cause: format!(
                            "form id for answer {} was not found while updating its search document",
                            content.answer_id
                        ),
                    }
                })?;
                Ok((
                    content.id,
                    AnswerContentSearchDocument {
                        id: content.id,
                        form_id,
                        answer_id: content.answer_id,
                        question_id: content.question_id,
                        answer: content.answer.clone(),
                    },
                ))
            })
            .collect::<Result<HashMap<_, _>, InfraError>>()?;

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
                    SearchableFields::AnswerTitle(answer) => {
                        self.meilisearch_client
                            .index("answers")
                            .add_or_replace(&[answer], Some("id"))
                            .await
                    }
                    SearchableFields::RealAnswers(answers) => {
                        self.meilisearch_client
                            .index("real_answers")
                            .add_or_replace(&[&content_documents[&answers.id]], Some("id"))
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
                    SearchableFields::AnswerTitle(answer) => {
                        self.meilisearch_client
                            .index("answers")
                            .delete_document(answer.id.to_string())
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

    #[tracing::instrument]
    async fn initialize_search_engine(&self) -> Result<bool, InfraError> {
        let index_with_uid = vec![
            ("form_meta_data", "id"),
            ("answers", "id"),
            ("real_answers", "id"),
            ("form_answer_comments", "id"),
            ("label_for_form_answers", "id"),
            ("label_for_forms", "id"),
            ("users", "id"),
        ];

        let futures = index_with_uid
            .into_iter()
            .map(async |(index, uid)| {
                self.meilisearch_client
                    .create_index(index, Some(uid))
                    .await?
                    .wait_for_completion(&self.meilisearch_client, None, None)
                    .await?;

                Ok::<_, meilisearch_sdk::errors::Error>(())
            })
            .collect_vec();

        futures::future::try_join_all(futures).await?;

        let settings_futures = ["answers", "real_answers"].into_iter().map(async |index| {
            self.meilisearch_client
                .index(index)
                .set_filterable_attributes(["form_id"])
                .await?
                .wait_for_completion(&self.meilisearch_client, None, None)
                .await?;

            Ok::<_, meilisearch_sdk::errors::Error>(())
        });
        futures::future::try_join_all(settings_futures).await?;

        answer_documents_need_reprojection(self).await
    }
}

#[cfg(test)]
mod tests {
    use super::{AnswerContentSearchDocument, form_filter, merge_answer_hits};
    use domain::form::{answer::AnswerId, models::FormId, question::QuestionId};
    use uuid::Uuid;

    fn answer_id(value: u128) -> AnswerId {
        Uuid::from_u128(value).into()
    }

    #[test]
    fn answer_form_filter_uses_the_form_id_attribute() {
        let form_id = FormId::from(Uuid::from_u128(1));

        assert_eq!(form_filter(form_id), format!("form_id = \"{form_id}\""));
    }

    #[test]
    fn answer_content_search_document_contains_form_id() {
        let form_id = FormId::from(Uuid::from_u128(1));
        let document = AnswerContentSearchDocument {
            id: Uuid::from_u128(2).into(),
            form_id,
            answer_id: answer_id(3),
            question_id: QuestionId::from(Uuid::from_u128(4)),
            answer: "content".to_string(),
        };

        assert_eq!(
            serde_json::to_value(document).unwrap()["form_id"],
            form_id.to_string()
        );
    }

    #[test]
    fn answer_search_hits_include_title_and_content_matches_with_title_first() {
        let title_match = answer_id(1);
        let content_match = answer_id(2);

        let hits = merge_answer_hits([title_match], [content_match]);

        assert_eq!(
            hits.into_iter()
                .map(|hit| hit.answer_id)
                .collect::<Vec<_>>(),
            vec![title_match, content_match]
        );
    }

    #[test]
    fn answer_search_hits_are_unique_when_title_and_content_both_match() {
        let answer_id = answer_id(1);

        let hits = merge_answer_hits([answer_id], [answer_id, answer_id]);

        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].answer_id, answer_id);
    }
}
