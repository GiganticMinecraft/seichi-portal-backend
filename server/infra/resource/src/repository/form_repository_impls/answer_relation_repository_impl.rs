use async_trait::async_trait;
use domain::{
    form::answer::{AnswerEntry, AnswerId},
    repository::form::answer_relation_repository::{
        AnswerRelationRepository, RelatedAnswerLifecycle, RelatedAnswerReference,
    },
    types::authorization_guard::{Allowed, Update},
};
use errors::{Error, domain::DomainError, infra::InfraError, usecase::UseCaseError};

use crate::{
    database::components::{DatabaseComponents, FormAnswerRelationDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client> AnswerRelationRepository for Repository<Client>
where
    Client: DatabaseComponents + 'static,
{
    async fn validate_replace_for_answer(
        &self,
        answer: &Allowed<AnswerEntry, Update>,
        related_answer_ids: &[AnswerId],
    ) -> Result<(), Error> {
        self.client
            .form_answer_relation()
            .validate_answer_relation_replacement(*answer.id(), related_answer_ids)
            .await
            .map_err(map_relation_replacement_error)
    }

    async fn replace_for_answer(
        &self,
        answer: Allowed<AnswerEntry, Update>,
        related_answer_ids: Vec<AnswerId>,
    ) -> Result<(), Error> {
        self.client
            .form_answer_relation()
            .replace_answer_relations(*answer.id(), related_answer_ids)
            .await
            .map_err(map_relation_replacement_error)
    }

    async fn update_answer_meta_and_replace(
        &self,
        answer: Allowed<AnswerEntry, Update>,
        related_answer_ids: Vec<AnswerId>,
    ) -> Result<(), Error> {
        self.client
            .form_answer_relation()
            .update_answer_meta_and_replace_relations(answer.value(), related_answer_ids)
            .await
            .map_err(map_relation_replacement_error)
    }

    async fn list_for_answer(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<RelatedAnswerReference>, Error> {
        Ok(self
            .client
            .form_answer_relation()
            .get_answer_relations(answer_id)
            .await?
            .into_iter()
            .map(|record| RelatedAnswerReference {
                form_id: record.form_id,
                answer_id: record.answer_id,
                lifecycle: if record.is_archived {
                    RelatedAnswerLifecycle::Archived
                } else {
                    RelatedAnswerLifecycle::Active
                },
            })
            .collect())
    }
}

fn map_relation_replacement_error(error: InfraError) -> Error {
    match error {
        InfraError::AnswerRelationSourceNotActive { .. } => UseCaseError::AnswerNotFound.into(),
        InfraError::AnswerRelationTargetNotActive { id } => DomainError::InvalidEntity {
            message: format!("related answer {id} does not exist or is archived"),
        }
        .into(),
        error => error.into(),
    }
}
