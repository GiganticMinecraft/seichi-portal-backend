use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::form::{models::FormId, question::models::Question};

#[automock]
#[async_trait]
pub trait QuestionRepository: Send + Sync + 'static {
    async fn create_questions(
        &self,
        form_id: FormId,
        questions: Vec<Question>,
    ) -> Result<(), Error>;
    async fn put_questions(&self, form_id: FormId, questions: Vec<Question>) -> Result<(), Error>;
    async fn get_questions(&self, form_id: FormId) -> Result<Vec<Question>, Error>;
}
