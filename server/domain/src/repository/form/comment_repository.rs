use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::types::authorization_guard_with_context::Update;
use crate::{
    form::{
        answer::models::AnswerId,
        comment::{
            models::{Comment, CommentId},
            service::CommentAuthorizationContext,
        },
    },
    types::authorization_guard_with_context::{
        AuthorizationGuardWithContext, Create, Delete, Read,
    },
    user::models::User,
};

#[automock]
#[async_trait]
pub trait CommentRepository: Send + Sync + 'static {
    async fn get_comments(
        &self,
        answer_id: AnswerId,
    ) -> Result<
        Vec<AuthorizationGuardWithContext<Comment, Read, CommentAuthorizationContext<Read>>>,
        Error,
    >;
    async fn get_all_comments(
        &self,
    ) -> Result<
        Vec<AuthorizationGuardWithContext<Comment, Read, CommentAuthorizationContext<Read>>>,
        Error,
    >;
    async fn get_comment(
        &self,
        comment_id: CommentId,
    ) -> Result<
        Option<AuthorizationGuardWithContext<Comment, Read, CommentAuthorizationContext<Read>>>,
        Error,
    >;
    async fn create_comment(
        &self,
        answer_id: AnswerId,
        context: &CommentAuthorizationContext<Read>,
        actor: &User,
        comment: AuthorizationGuardWithContext<Comment, Create, CommentAuthorizationContext<Read>>,
    ) -> Result<(), Error>;
    async fn update_comment(
        &self,
        answer_id: AnswerId,
        context: &CommentAuthorizationContext<Read>,
        actor: &User,
        comment: AuthorizationGuardWithContext<Comment, Update, CommentAuthorizationContext<Read>>,
    ) -> Result<(), Error>;
    async fn delete_comment(
        &self,
        context: CommentAuthorizationContext<Read>,
        actor: &User,
        comment: AuthorizationGuardWithContext<Comment, Delete, CommentAuthorizationContext<Read>>,
    ) -> Result<(), Error>;
    async fn size(&self) -> Result<u32, Error>;
}
