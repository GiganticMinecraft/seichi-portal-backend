use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::form::{
    answer::models::AnswerId,
    comment::models::{Comment, CommentId},
};

#[automock]
#[async_trait]
pub trait CommentRepository: Send + Sync + 'static {
    async fn get_comments(&self, answer_id: AnswerId) -> Result<Vec<Comment>, Error>;
    async fn post_comment(&self, answer_id: AnswerId, comment: &Comment) -> Result<(), Error>;
    async fn delete_comment(&self, comment_id: CommentId) -> Result<(), Error>;
}
