use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::form::models::Answer;
use crate::{
    form::models::{Form, Label},
    user::models::User,
};

#[automock]
#[async_trait]
pub trait SearchRepository: Send + Sync + 'static {
    async fn search_users(&self, query: String) -> Result<Vec<User>, Error>;
    async fn search_forms(&self, query: String) -> Result<Vec<Form>, Error>;
    async fn search_labels_for_forms(&self, query: String) -> Result<Vec<Label>, Error>;
    async fn search_labels_for_answers(&self, query: String) -> Result<Vec<Label>, Error>;
    async fn search_answers(&self, query: String) -> Result<Vec<Answer>, Error>;
}
