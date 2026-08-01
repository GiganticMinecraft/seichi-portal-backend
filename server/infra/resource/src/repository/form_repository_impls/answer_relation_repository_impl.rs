use async_trait::async_trait;
use domain::{
    form::answer::{
        AnswerEntry, AnswerId, AnswerReference, AnswerRelation, AnswerRelationEndpoint,
        ArchivedAnswerEntry, ReadableAnswerRelation,
    },
    form::models::{ActiveForm, ArchivedForm},
    repository::form::answer_relation_repository::AnswerRelationRepository,
    types::authorization_guard::{Allowed, AuthorizationGuard, Read, Update},
};
use errors::{Error, domain::DomainError};

use crate::{
    database::components::{
        DatabaseComponents, FormAnswerDatabase, FormAnswerRelationDatabase, FormDatabase,
    },
    repository::Repository,
};

fn ensure_relation_matches_answers(
    relation: AnswerRelation,
    source: &Allowed<AnswerEntry, Update>,
    target: &Allowed<AnswerEntry, Update>,
) -> Result<(), Error> {
    if source.actor() != target.actor() {
        return Err(DomainError::InvalidEntity {
            message: "answer relation proofs must belong to the same actor".to_string(),
        }
        .into());
    }
    if !relation.connects(source.value(), target.value()) {
        return Err(DomainError::InvalidEntity {
            message: "answer relation endpoints do not match authorized answers".to_string(),
        }
        .into());
    }
    Ok(())
}

enum ReadableAnswer {
    Active(Allowed<AnswerEntry, Read>),
    Archived(Allowed<ArchivedAnswerEntry, Read>),
}

async fn load_active_answer<Client>(
    repository: &Repository<Client>,
    actor: &domain::auth::Actor,
    reference: AnswerReference,
) -> Result<Option<Allowed<AnswerEntry, Read>>, Error>
where
    Client: DatabaseComponents + 'static,
{
    let Some(form_record) = repository.client.form().get(reference.form_id()).await? else {
        return Ok(None);
    };
    let form = TryInto::<ActiveForm>::try_into(form_record)?;
    let form = match AuthorizationGuard::<ActiveForm, Read>::from(form).try_read(actor.clone()) {
        Ok(form) => form,
        Err(DomainError::Forbidden) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let Some(answer_record) = repository
        .client
        .form_answer()
        .get_answers(reference.answer_id())
        .await?
    else {
        return Ok(None);
    };
    let answer = TryInto::<AnswerEntry>::try_into(answer_record)?;
    match form.read_entry(answer) {
        Ok(answer) => Ok(Some(answer)),
        Err(DomainError::Forbidden | DomainError::NotFound) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

async fn load_archived_answer<Client>(
    repository: &Repository<Client>,
    actor: &domain::auth::Actor,
    reference: AnswerReference,
) -> Result<Option<Allowed<ArchivedAnswerEntry, Read>>, Error>
where
    Client: DatabaseComponents + 'static,
{
    let Some(form_record) = repository
        .client
        .form()
        .get_archived(reference.form_id())
        .await?
    else {
        return Ok(None);
    };
    let form = TryInto::<ArchivedForm>::try_into(form_record)?;
    let form = match AuthorizationGuard::<ArchivedForm, Read>::from(form).try_read(actor.clone()) {
        Ok(form) => form,
        Err(DomainError::Forbidden) => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let Some(publication) = repository
        .client
        .form()
        .archived_answer_publication(*form.form().id(), reference.answer_id())
        .await?
    else {
        return Ok(None);
    };
    let answer = unsafe {
        ArchivedAnswerEntry::from_raw_parts(reference.answer_id(), *form.form().id(), publication)
    };
    match form.read_archived_entry(answer) {
        Ok(answer) => Ok(Some(answer)),
        Err(DomainError::Forbidden | DomainError::NotFound) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

async fn load_target<Client>(
    repository: &Repository<Client>,
    actor: &domain::auth::Actor,
    reference: AnswerReference,
) -> Result<Option<ReadableAnswer>, Error>
where
    Client: DatabaseComponents + 'static,
{
    if let Some(answer) = load_active_answer(repository, actor, reference).await? {
        return Ok(Some(ReadableAnswer::Active(answer)));
    }
    Ok(load_archived_answer(repository, actor, reference)
        .await?
        .map(ReadableAnswer::Archived))
}

async fn list_for_reference<Client, Source>(
    repository: &Repository<Client>,
    source: &Allowed<Source, Read>,
    source_reference: AnswerReference,
) -> Result<Vec<Allowed<ReadableAnswerRelation, Read>>, Error>
where
    Client: DatabaseComponents + 'static,
    Source: AnswerRelationEndpoint,
{
    let records = repository
        .client
        .form_answer_relation()
        .list_for_answer(source_reference)
        .await?;
    let mut relations = Vec::with_capacity(records.len());
    for record in records {
        let relation = AnswerRelation::new(record.first, record.second).map_err(|error| {
            Error::from(DomainError::InvalidEntity {
                message: error.to_string(),
            })
        })?;
        let Some(target_reference) = relation.other_endpoint(source_reference) else {
            continue;
        };
        let Some(target) = load_target(repository, source.actor(), target_reference).await? else {
            continue;
        };
        let relation = match target {
            ReadableAnswer::Active(target) => relation.authorize_read(source, &target),
            ReadableAnswer::Archived(target) => relation.authorize_read(source, &target),
        }?;
        relations.push(relation);
    }
    Ok(relations)
}

#[async_trait]
impl<Client> AnswerRelationRepository for Repository<Client>
where
    Client: DatabaseComponents + 'static,
{
    #[tracing::instrument(skip_all)]
    async fn list_for_answer(
        &self,
        source: &Allowed<AnswerEntry, Read>,
    ) -> Result<Vec<Allowed<ReadableAnswerRelation, Read>>, Error> {
        list_for_reference(
            self,
            source,
            AnswerReference::new(*source.form_id(), *source.id()),
        )
        .await
    }

    #[tracing::instrument(skip_all)]
    async fn list_for_archived_answer(
        &self,
        source: &Allowed<ArchivedAnswerEntry, Read>,
    ) -> Result<Vec<Allowed<ReadableAnswerRelation, Read>>, Error> {
        list_for_reference(
            self,
            source,
            AnswerReference::new(*source.form_id(), *source.id()),
        )
        .await
    }

    #[tracing::instrument(skip_all)]
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

    #[tracing::instrument(skip_all)]
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

    #[tracing::instrument(skip_all, fields(answer_id = %answer_id))]
    async fn find_for_source_and_answer_id(
        &self,
        source: &Allowed<AnswerEntry, Update>,
        answer_id: AnswerId,
    ) -> Result<Option<Allowed<ReadableAnswerRelation, Read>>, Error> {
        let source_reference = AnswerReference::new(*source.form_id(), *source.id());
        let Some(record) = self
            .client
            .form_answer_relation()
            .find_for_source_and_answer_id(source_reference, answer_id)
            .await?
        else {
            return Ok(None);
        };
        let relation = AnswerRelation::new(record.first, record.second).map_err(|error| {
            Error::from(DomainError::InvalidEntity {
                message: error.to_string(),
            })
        })?;
        let Some(target_reference) = relation.other_endpoint(source_reference) else {
            return Ok(None);
        };
        let Some(target) = load_target(self, source.actor(), target_reference).await? else {
            return Ok(None);
        };
        let relation = match target {
            ReadableAnswer::Active(target) => relation.authorize_read_from_update(source, &target),
            ReadableAnswer::Archived(target) => {
                relation.authorize_read_from_update(source, &target)
            }
        }?;
        Ok(Some(relation))
    }
}
