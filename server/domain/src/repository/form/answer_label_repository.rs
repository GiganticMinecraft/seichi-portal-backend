use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::form::answer::models::{AnswerId, AnswerLabel, AnswerLabelId};

#[automock]
#[async_trait]
pub trait AnswerLabelRepository: Send + Sync + 'static {
    async fn create_label_for_answers(&self, label_name: String) -> Result<(), Error>;
    async fn get_labels_for_answers(&self) -> Result<Vec<AnswerLabel>, Error>;
    async fn get_labels_for_answers_by_answer_id(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<AnswerLabel>, Error>;
    async fn delete_label_for_answers(&self, label_id: AnswerLabelId) -> Result<(), Error>;
    async fn edit_label_for_answers(&self, label: &AnswerLabel) -> Result<(), Error>;
    async fn replace_answer_labels(
        &self,
        answer_id: AnswerId,
        label_ids: Vec<AnswerLabelId>,
    ) -> Result<(), Error>;
}
