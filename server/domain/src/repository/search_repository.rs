use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::{
        answer::models::{AnswerLabel, FormAnswerContent},
        comment::{models::Comment, service::CommentAuthorizationContext},
        models::{Form, FormLabel},
    },
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard_with_context::{AuthorizationGuardWithContext, Read},
    },
    user::models::User,
};

#[automock]
#[async_trait]
pub trait SearchRepository: Send + Sync + 'static {
    async fn search_users(&self, query: &str)
        -> Result<Vec<AuthorizationGuard<User, Read>>, Error>;
    async fn search_forms(&self, query: &str)
        -> Result<Vec<AuthorizationGuard<Form, Read>>, Error>;
    async fn search_labels_for_forms(
        &self,
        query: &str,
    ) -> Result<Vec<AuthorizationGuard<FormLabel, Read>>, Error>;
    async fn search_labels_for_answers(
        &self,
        query: &str,
    ) -> Result<Vec<AuthorizationGuard<AnswerLabel, Read>>, Error>;
    async fn search_answers(&self, query: &str) -> Result<Vec<FormAnswerContent>, Error>;
    async fn search_comments(
        &self,
        query: &str,
    ) -> Result<
        Vec<AuthorizationGuardWithContext<Comment, Read, CommentAuthorizationContext<Read>>>,
        Error,
    >;
}
