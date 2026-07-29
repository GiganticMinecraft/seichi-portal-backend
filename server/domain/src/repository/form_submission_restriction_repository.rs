use async_trait::async_trait;
use errors::Error;
use mockall::automock;
use uuid::Uuid;

use crate::{
    form::{FormSubmissionRestriction, FormSubmissionRestrictionHistory},
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Delete, Read},
};

#[automock]
#[async_trait]
pub trait FormSubmissionRestrictionRepository: Send + Sync + 'static {
    async fn fetch_active_by_submitter_id(
        &self,
        submitter_id: Uuid,
    ) -> Result<Option<AuthorizationGuard<FormSubmissionRestriction, Read>>, Error>;

    async fn list_by_submitter_id(
        &self,
        submitter_id: Uuid,
    ) -> Result<AuthorizationGuard<FormSubmissionRestrictionHistory, Read>, Error>;

    async fn restrict(
        &self,
        restriction: Allowed<FormSubmissionRestriction, Create>,
    ) -> Result<(), Error>;

    async fn lift(
        &self,
        restriction: Allowed<FormSubmissionRestriction, Delete>,
    ) -> Result<(), Error>;
}
