use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::{models::FormId, question::models::Question},
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard_with_context::{Create, Read, Update},
    },
    user::models::User,
};

#[automock]
#[async_trait]
pub trait QuestionRepository: Send + Sync + 'static {
    async fn create_questions(
        &self,
        actor: &User,
        form_id: FormId,
        questions: Vec<AuthorizationGuard<Question, Create>>,
    ) -> Result<(), Error>;
    async fn put_questions(
        &self,
        actor: &User,
        form_id: FormId,
        questions: Vec<AuthorizationGuard<Question, Update>>,
    ) -> Result<(), Error>;
    async fn get_questions(
        &self,
        form_id: FormId,
    ) -> Result<Vec<AuthorizationGuard<Question, Read>>, Error>;
}
