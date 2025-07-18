use crate::{
    database::components::{DatabaseComponents, FormCommentDatabase},
    repository::Repository,
};
use async_trait::async_trait;
use domain::types::authorization_guard_with_context::Update;
use domain::{
    form::{
        answer::models::AnswerId,
        comment::{
            models::{Comment, CommentId},
            service::CommentAuthorizationContext,
        },
    },
    repository::form::comment_repository::CommentRepository,
    types::authorization_guard_with_context::{
        AuthorizationGuardWithContext, Create, Delete, Read,
    },
    user::models::User,
};
use errors::Error;
use itertools::Itertools;

#[async_trait]
impl<Client: DatabaseComponents + 'static> CommentRepository for Repository<Client> {
    #[tracing::instrument(skip(self))]
    async fn get_comments(
        &self,
        answer_id: AnswerId,
    ) -> Result<
        Vec<AuthorizationGuardWithContext<Comment, Read, CommentAuthorizationContext<Read>>>,
        Error,
    > {
        self.client
            .form_comment()
            .get_comments(answer_id)
            .await?
            .into_iter()
            .map(TryInto::<Comment>::try_into)
            .map_ok(|comment| AuthorizationGuardWithContext::new(comment).into_read())
            .collect::<Result<Vec<_>, _>>()
    }

    #[tracing::instrument(skip(self))]
    async fn get_all_comments(
        &self,
    ) -> Result<
        Vec<AuthorizationGuardWithContext<Comment, Read, CommentAuthorizationContext<Read>>>,
        Error,
    > {
        self.client
            .form_comment()
            .get_all_comments()
            .await?
            .into_iter()
            .map(TryInto::<Comment>::try_into)
            .map_ok(|comment| AuthorizationGuardWithContext::new(comment).into_read())
            .collect::<Result<Vec<_>, _>>()
    }

    #[tracing::instrument(skip(self))]
    async fn get_comment(
        &self,
        comment_id: CommentId,
    ) -> Result<
        Option<AuthorizationGuardWithContext<Comment, Read, CommentAuthorizationContext<Read>>>,
        Error,
    > {
        Ok(self
            .client
            .form_comment()
            .get_comment(comment_id)
            .await?
            .map(TryInto::<Comment>::try_into)
            .transpose()?
            .map(|comment| AuthorizationGuardWithContext::new(comment).into_read()))
    }

    #[tracing::instrument(skip(self))]
    async fn create_comment(
        &self,
        answer_id: AnswerId,
        context: &CommentAuthorizationContext<Read>,
        actor: &User,
        comment: AuthorizationGuardWithContext<Comment, Create, CommentAuthorizationContext<Read>>,
    ) -> Result<(), Error> {
        comment
            .try_create(
                actor,
                |comment| {
                    self.client
                        .form_comment()
                        .upsert_comment(answer_id, comment)
                },
                context,
            )?
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn update_comment(
        &self,
        answer_id: AnswerId,
        context: &CommentAuthorizationContext<Read>,
        actor: &User,
        comment: AuthorizationGuardWithContext<Comment, Update, CommentAuthorizationContext<Read>>,
    ) -> Result<(), Error> {
        comment
            .try_update(
                actor,
                |comment| {
                    self.client
                        .form_comment()
                        .upsert_comment(answer_id, comment)
                },
                context,
            )?
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn delete_comment(
        &self,
        context: CommentAuthorizationContext<Read>,
        actor: &User,
        comment: AuthorizationGuardWithContext<Comment, Delete, CommentAuthorizationContext<Read>>,
    ) -> Result<(), Error> {
        comment
            .try_delete(
                actor,
                |comment| {
                    self.client
                        .form_comment()
                        .delete_comment(comment.comment_id().to_owned())
                },
                &context,
            )?
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn size(&self) -> Result<u32, Error> {
        self.client.form_comment().size().await.map_err(Into::into)
    }
}
