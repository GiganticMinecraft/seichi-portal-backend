use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::form::{
    answer::models::{AnswerEntry, AnswerId},
    models::FormId,
};

#[automock]
#[async_trait]
pub trait AnswerRepository: Send + Sync + 'static {
    async fn post_answer(&self, answer: &AnswerEntry) -> Result<(), Error>;
    async fn get_answer(&self, answer_id: AnswerId) -> Result<Option<AnswerEntry>, Error>;
    async fn get_answers_by_form_id(&self, form_id: FormId) -> Result<Vec<AnswerEntry>, Error>;
    async fn get_all_answers(&self) -> Result<Vec<AnswerEntry>, Error>;
    async fn update_answer_entry(&self, answer_entry: &AnswerEntry) -> Result<(), Error>;
    async fn size(&self) -> Result<u32, Error>;
}
