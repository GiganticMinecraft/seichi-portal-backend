use async_trait::async_trait;
use domain::{
    form::{
        answer::models::AnswerId,
        comment::models::{Comment, CommentId},
    },
    repository::form::comment_repository::CommentRepository,
};
use errors::Error;

use crate::{
    database::components::{DatabaseComponents, FormCommentDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> CommentRepository for Repository<Client> {
    #[tracing::instrument(skip(self))]
    async fn get_comments(&self, answer_id: AnswerId) -> Result<Vec<Comment>, Error> {
        self.client
            .form_comment()
            .get_comments(answer_id)
            .await
            .map(|comments| {
                comments
                    .into_iter()
                    .map(|comment_dto| comment_dto.try_into())
                    .collect::<Result<Vec<Comment>, _>>()
            })?
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn post_comment(&self, answer_id: AnswerId, comment: &Comment) -> Result<(), Error> {
        self.client
            .form_comment()
            .post_comment(answer_id, comment)
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
}
