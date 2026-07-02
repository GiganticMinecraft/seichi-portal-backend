use async_trait::async_trait;
use errors::Error;
use mockall::automock;
use uuid::Uuid;

use crate::{
    form::answer::{AnswerSubmitterRestriction, AnswerSubmitterRestrictionHistory},
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Delete, Read},
};

#[automock]
#[async_trait]
pub trait AnswerSubmitterRestrictionRepository: Send + Sync + 'static {
    async fn fetch_active_by_submitter_id(
        &self,
        submitter_id: Uuid,
    ) -> Result<Option<AuthorizationGuard<AnswerSubmitterRestriction, Read>>, Error>;

    async fn list_by_submitter_id(
        &self,
        submitter_id: Uuid,
    ) -> Result<AuthorizationGuard<AnswerSubmitterRestrictionHistory, Read>, Error>;

    async fn restrict(
        &self,
        restriction: Allowed<AnswerSubmitterRestriction, Create>,
    ) -> Result<(), Error>;

    async fn lift(
        &self,
        restriction: Allowed<AnswerSubmitterRestriction, Delete>,
    ) -> Result<(), Error>;
}
