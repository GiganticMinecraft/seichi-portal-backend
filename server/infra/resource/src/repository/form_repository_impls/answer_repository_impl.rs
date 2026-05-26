use async_trait::async_trait;
use domain::{
    form::answer::models::AnswerEntry, repository::form::answer_repository::AnswerRepository,
};
use errors::Error;

use crate::{
    database::components::{DatabaseComponents, FormAnswerDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> AnswerRepository for Repository<Client> {
    #[tracing::instrument(skip(self))]
    async fn post_answer(&self, answer: &AnswerEntry) -> Result<(), Error> {
        self.client
            .form_answer()
            .post_answer(answer)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn update_answer_entry(&self, answer_entry: &AnswerEntry) -> Result<(), Error> {
        self.client
            .form_answer()
            .update_answer_entry(answer_entry)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn size(&self) -> Result<u32, Error> {
        self.client.form_answer().size().await.map_err(Into::into)
    }
}
