use async_trait::async_trait;
use domain::{
    auth::Actor,
    form::answer::AnswerSubmitterRestriction,
    repository::answer_submitter_restriction_repository::AnswerSubmitterRestrictionRepository,
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Delete, Read},
};
use errors::{Error, domain::DomainError};
use uuid::Uuid;

use crate::{
    database::components::{AnswerSubmitterRestrictionDatabase, DatabaseComponents},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> AnswerSubmitterRestrictionRepository
    for Repository<Client>
{
    async fn fetch_active_by_submitter_id(
        &self,
        submitter_id: Uuid,
    ) -> Result<Option<AuthorizationGuard<AnswerSubmitterRestriction, Read>>, Error> {
        Ok(self
            .client
            .answer_submitter_restriction()
            .fetch_active_by_submitter_id(submitter_id)
            .await?
            .map(Into::into))
    }

    async fn restrict(
        &self,
        restriction: Allowed<AnswerSubmitterRestriction, Create>,
    ) -> Result<(), Error> {
        self.client
            .answer_submitter_restriction()
            .restrict(restriction.value())
            .await
            .map_err(Into::into)
    }

    async fn lift(
        &self,
        restriction: Allowed<AnswerSubmitterRestriction, Delete>,
    ) -> Result<(), Error> {
        let lifted_by = match restriction.actor() {
            Actor::AccountUser(user) => user.id().into_inner(),
            _ => return Err(DomainError::Forbidden.into()),
        };

        self.client
            .answer_submitter_restriction()
            .lift(restriction.submitter_id().into_inner(), lifted_by)
            .await
            .map_err(Into::into)
    }
}
