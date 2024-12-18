use domain::{
    form::{
        answer::models::AnswerId,
        models::{Label, LabelId},
    },
    repository::form::answer_label_repository::AnswerLabelRepository,
};
use errors::Error;

pub struct AnswerLabelUseCase<'a, AnswerLabelRepo: AnswerLabelRepository> {
    pub answer_label_repository: &'a AnswerLabelRepo,
}

impl<R1: AnswerLabelRepository> AnswerLabelUseCase<'_, R1> {
    pub async fn create_label_for_answers(&self, label_name: String) -> Result<(), Error> {
        self.answer_label_repository
            .create_label_for_answers(label_name)
            .await
    }

    pub async fn get_labels_for_answers(&self) -> Result<Vec<Label>, Error> {
        self.answer_label_repository.get_labels_for_answers().await
    }

    pub async fn delete_label_for_answers(&self, label_id: LabelId) -> Result<(), Error> {
        self.answer_label_repository
            .delete_label_for_answers(label_id)
            .await
    }

    pub async fn edit_label_for_answers(&self, label_schema: &Label) -> Result<(), Error> {
        self.answer_label_repository
            .edit_label_for_answers(label_schema)
            .await
    }

    pub async fn replace_answer_labels(
        &self,
        answer_id: AnswerId,
        label_ids: Vec<LabelId>,
    ) -> Result<(), Error> {
        self.answer_label_repository
            .replace_answer_labels(answer_id, label_ids)
            .await
    }
}
