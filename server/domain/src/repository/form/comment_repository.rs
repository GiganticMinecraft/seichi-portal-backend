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
    async fn get_all_comments(&self) -> Result<Vec<Comment>, Error>;
    async fn get_comment(&self, comment_id: CommentId) -> Result<Option<Comment>, Error>;
    async fn create_comment(&self, answer_id: AnswerId, comment: &Comment) -> Result<(), Error>;
    async fn update_comment(&self, answer_id: AnswerId, comment: &Comment) -> Result<(), Error>;
    async fn delete_comment(&self, comment_id: CommentId) -> Result<(), Error>;
    async fn size(&self) -> Result<u32, Error>;
}
