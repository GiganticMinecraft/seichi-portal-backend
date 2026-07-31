use async_trait::async_trait;
use domain::{
    form::answer::{AnswerEntry, AnswerId, AnswerReference, AnswerRelation},
    form::models::ArchivedForm,
    repository::form::answer_relation_repository::AnswerRelationRepository,
    types::authorization_guard::{Allowed, Read, Update},
};
use errors::{Error, domain::DomainError};

use crate::{
    database::components::{DatabaseComponents, FormAnswerRelationDatabase},
    repository::Repository,
};

fn ensure_relation_matches_answers(
    relation: AnswerRelation,
    source: &Allowed<AnswerEntry, Update>,
    target: &Allowed<AnswerEntry, Update>,
) -> Result<(), Error> {
    let source_reference = AnswerReference::new(*source.form_id(), *source.id());
    let target_reference = AnswerReference::new(*target.form_id(), *target.id());
    if relation.other_endpoint(source_reference) != Some(target_reference) {
        return Err(DomainError::InvalidEntity {
            message: "answer relation endpoints do not match authorized answers".to_string(),
        }
        .into());
    }
    Ok(())
}

async fn list_for_reference<Client>(
    repository: &Repository<Client>,
    source_reference: AnswerReference,
) -> Result<Vec<AnswerRelation>, Error>
where
    Client: DatabaseComponents + 'static,
{
    repository
        .client
        .form_answer_relation()
        .list_for_answer(source_reference)
        .await?
        .into_iter()
        .map(|record| {
            AnswerRelation::new(record.first, record.second)
                .map_err(|error| DomainError::InvalidEntity {
                    message: error.to_string(),
                })
                .map_err(Into::into)
        })
        .collect()
}

#[async_trait]
impl<Client> AnswerRelationRepository for Repository<Client>
where
    Client: DatabaseComponents + 'static,
{
    #[tracing::instrument(skip(self, source))]
    async fn list_for_answer(
        &self,
        source: &Allowed<AnswerEntry, Read>,
    ) -> Result<Vec<AnswerRelation>, Error> {
        list_for_reference(self, AnswerReference::new(*source.form_id(), *source.id())).await
    }

    #[tracing::instrument(skip(self))]
    async fn list_for_archived_answer(
        &self,
        source: &Allowed<ArchivedForm, Read>,
        answer_id: AnswerId,
    ) -> Result<Vec<AnswerRelation>, Error> {
        list_for_reference(self, AnswerReference::new(*source.form().id(), answer_id)).await
    }

    #[tracing::instrument(skip(self, source, target))]
    async fn add(
        &self,
        relation: AnswerRelation,
        source: &Allowed<AnswerEntry, Update>,
        target: &Allowed<AnswerEntry, Update>,
    ) -> Result<(), Error> {
        ensure_relation_matches_answers(relation, source, target)?;
        self.client
            .form_answer_relation()
            .add(relation)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self, source, target))]
    async fn remove(
        &self,
        relation: AnswerRelation,
        source: &Allowed<AnswerEntry, Update>,
        target: &Allowed<AnswerEntry, Update>,
    ) -> Result<(), Error> {
        ensure_relation_matches_answers(relation, source, target)?;
        self.client
            .form_answer_relation()
            .remove(relation)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn find_for_source_and_answer_id(
        &self,
        source: &Allowed<AnswerEntry, Update>,
        answer_id: AnswerId,
    ) -> Result<Option<AnswerRelation>, Error> {
        let source = AnswerReference::new(*source.form_id(), *source.id());
        let record = self
            .client
            .form_answer_relation()
            .find_for_source_and_answer_id(source, answer_id)
            .await?;
        record
            .map(|record| {
                AnswerRelation::new(record.first, record.second).map_err(|error| {
                    Error::from(DomainError::InvalidEntity {
                        message: error.to_string(),
                    })
                })
            })
            .transpose()
    }
}
