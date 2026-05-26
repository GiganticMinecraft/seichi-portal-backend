use crate::{
    database::components::{DatabaseComponents, FormCommentDatabase},
    repository::Repository,
};
use async_trait::async_trait;
use domain::{
    form::{
        answer::models::AnswerId,
        comment::models::{Comment, CommentId},
    },
    repository::form::comment_repository::CommentRepository,
};
use errors::Error;

#[async_trait]
impl<Client: DatabaseComponents + 'static> CommentRepository for Repository<Client> {
    #[tracing::instrument(skip(self))]
    async fn get_comments(&self, answer_id: AnswerId) -> Result<Vec<Comment>, Error> {
        self.client
            .form_comment()
            .get_comments(answer_id)
            .await?
            .into_iter()
            .map(TryInto::<Comment>::try_into)
            .collect()
    }

    #[tracing::instrument(skip(self))]
    async fn get_all_comments(&self) -> Result<Vec<Comment>, Error> {
        self.client
            .form_comment()
            .get_all_comments()
            .await?
            .into_iter()
            .map(TryInto::<Comment>::try_into)
            .collect()
    }

    #[tracing::instrument(skip(self))]
    async fn get_comment(&self, comment_id: CommentId) -> Result<Option<Comment>, Error> {
        self.client
            .form_comment()
            .get_comment(comment_id)
            .await?
            .map(TryInto::<Comment>::try_into)
            .transpose()
    }

    #[tracing::instrument(skip(self))]
    async fn create_comment(&self, answer_id: AnswerId, comment: &Comment) -> Result<(), Error> {
        self.client
            .form_comment()
            .upsert_comment(answer_id, comment)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn update_comment(&self, answer_id: AnswerId, comment: &Comment) -> Result<(), Error> {
        self.client
            .form_comment()
            .upsert_comment(answer_id, comment)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn delete_comment(&self, comment_id: CommentId) -> Result<(), Error> {
        self.client
            .form_comment()
            .delete_comment(comment_id)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn size(&self) -> Result<u32, Error> {
        self.client.form_comment().size().await.map_err(Into::into)
    }
}
