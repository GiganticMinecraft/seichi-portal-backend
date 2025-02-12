use domain::user::models::User;
use domain::{
    form::answer::models::{AnswerId, AnswerLabel, AnswerLabelId},
    repository::form::answer_label_repository::AnswerLabelRepository,
};
use errors::usecase::UseCaseError::LabelNotFound;
use errors::Error;

pub struct AnswerLabelUseCase<'a, AnswerLabelRepo: AnswerLabelRepository> {
    pub answer_label_repository: &'a AnswerLabelRepo,
}

impl<R1: AnswerLabelRepository> AnswerLabelUseCase<'_, R1> {
    pub async fn create_label_for_answers(
        &self,
        actor: &User,
        label_name: String,
    ) -> Result<(), Error> {
        let answer_label = AnswerLabel::new(label_name);

        self.answer_label_repository
            .create_label_for_answers(actor, answer_label.into())
            .await
    }

    pub async fn get_labels_for_answers(&self, actor: &User) -> Result<Vec<AnswerLabel>, Error> {
        Ok(self
            .answer_label_repository
            .get_labels_for_answers()
            .await?
            .into_iter()
            .flat_map(|label| label.try_into_read(actor))
            .collect::<Vec<_>>())
    }

    pub async fn delete_label_for_answers(
        &self,
        actor: &User,
        label_id: AnswerLabelId,
    ) -> Result<(), Error> {
        let answer_label = self
            .answer_label_repository
            .get_label_for_answers(label_id)
            .await?
            .ok_or(Error::from(LabelNotFound))?;

        self.answer_label_repository
            .delete_label_for_answers(actor, answer_label.into_delete())
            .await
    }

    pub async fn edit_label_for_answers(
        &self,
        actor: &User,
        label: AnswerLabel,
    ) -> Result<(), Error> {
        self.answer_label_repository
            .edit_label_for_answers(actor, label.into())
            .await
    }

    pub async fn replace_answer_labels(
        &self,
        answer_id: AnswerId,
        label_ids: Vec<AnswerLabelId>,
    ) -> Result<(), Error> {
        self.answer_label_repository
            .replace_answer_labels(answer_id, label_ids)
            .await
    }
}
