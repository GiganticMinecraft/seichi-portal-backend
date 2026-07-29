use async_trait::async_trait;
use domain::{
    auth::Actor,
    form::{FormSubmissionRestriction, FormSubmissionRestrictionHistory},
    repository::form_submission_restriction_repository::FormSubmissionRestrictionRepository,
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Delete, Read},
};
use errors::{Error, domain::DomainError};
use uuid::Uuid;

use crate::{
    database::components::{DatabaseComponents, FormSubmissionRestrictionDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> FormSubmissionRestrictionRepository
    for Repository<Client>
{
    async fn fetch_active_by_submitter_id(
        &self,
        submitter_id: Uuid,
    ) -> Result<Option<AuthorizationGuard<FormSubmissionRestriction, Read>>, Error> {
        Ok(self
            .client
            .form_submission_restriction()
            .fetch_active_by_submitter_id(submitter_id)
            .await?
            .map(Into::into))
    }

    async fn list_by_submitter_id(
        &self,
        submitter_id: Uuid,
    ) -> Result<AuthorizationGuard<FormSubmissionRestrictionHistory, Read>, Error> {
        Ok(FormSubmissionRestrictionHistory::new(
            submitter_id.into(),
            self.client
                .form_submission_restriction()
                .list_by_submitter_id(submitter_id)
                .await?,
        )?
        .into())
    }

    async fn restrict(
        &self,
        restriction: Allowed<FormSubmissionRestriction, Create>,
    ) -> Result<(), Error> {
        self.client
            .form_submission_restriction()
            .restrict(restriction.value())
            .await
            .map_err(Into::into)
    }

    async fn lift(
        &self,
        restriction: Allowed<FormSubmissionRestriction, Delete>,
    ) -> Result<(), Error> {
        let lifted_by = match restriction.actor() {
            Actor::AccountUser(user) => user.id().into_inner(),
            _ => return Err(DomainError::Forbidden.into()),
        };

        self.client
            .form_submission_restriction()
            .lift(restriction.submitter_id().into_inner(), lifted_by)
            .await
            .map_err(Into::into)
    }
}
