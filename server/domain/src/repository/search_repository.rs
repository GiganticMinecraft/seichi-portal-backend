use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::models::{Answer, Comment, Form, Label},
    user::models::User,
};

#[automock]
#[async_trait]
pub trait SearchRepository: Send + Sync + 'static {
    async fn search_users(&self, query: &str) -> Result<Vec<User>, Error>;
    async fn search_forms(&self, query: &str) -> Result<Vec<Form>, Error>;
    async fn search_labels_for_forms(&self, query: &str) -> Result<Vec<Label>, Error>;
    async fn search_labels_for_answers(&self, query: &str) -> Result<Vec<Label>, Error>;
    async fn search_answers(&self, query: &str) -> Result<Vec<Answer>, Error>;
    async fn search_comments(&self, query: &str) -> Result<Vec<Comment>, Error>;
}
