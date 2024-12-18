use async_trait::async_trait;
use domain::{
    form::{
        answer::models::{AnswerId, AnswerLabel},
        models::{Label, LabelId},
    },
    repository::form::answer_label_repository::AnswerLabelRepository,
};
use errors::Error;
use futures::{stream, StreamExt};

use crate::{
    database::components::{DatabaseComponents, FormAnswerLabelDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> AnswerLabelRepository for Repository<Client> {
    #[tracing::instrument(skip(self))]
    async fn create_label_for_answers(&self, label_name: String) -> Result<(), Error> {
        self.client
            .form_answer_label()
            .create_label_for_answers(label_name)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn get_labels_for_answers(&self) -> Result<Vec<Label>, Error> {
        stream::iter(
            self.client
                .form_answer_label()
                .get_labels_for_answers()
                .await?,
        )
        .then(|label_dto| async { Ok(label_dto.try_into()?) })
        .collect::<Vec<Result<Label, _>>>()
        .await
        .into_iter()
        .collect::<Result<Vec<Label>, _>>()
    }

    #[tracing::instrument(skip(self))]
    async fn get_labels_for_answers_by_answer_id(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<AnswerLabel>, Error> {
        self.client
            .form_answer_label()
            .get_labels_for_answers_by_answer_id(answer_id)
            .await
            .map(|labels| {
                labels
                    .into_iter()
                    .map(|label_dto| label_dto.try_into())
                    .collect::<Result<Vec<AnswerLabel>, _>>()
            })?
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn delete_label_for_answers(&self, label_id: LabelId) -> Result<(), Error> {
        self.client
            .form_answer_label()
            .delete_label_for_answers(label_id)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn edit_label_for_answers(&self, label: &Label) -> Result<(), Error> {
        self.client
            .form_answer_label()
            .edit_label_for_answers(label)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn replace_answer_labels(
        &self,
        answer_id: AnswerId,
        label_ids: Vec<LabelId>,
    ) -> Result<(), Error> {
        self.client
            .form_answer_label()
            .replace_answer_labels(answer_id, label_ids)
            .await
            .map_err(Into::into)
    }
}
