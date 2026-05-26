use async_trait::async_trait;
use domain::{
    form::{
        answer::models::AnswerId,
        message::models::{Message, MessageId},
    },
    repository::form::message_repository::MessageRepository,
};
use errors::Error;

use crate::{
    database::components::{DatabaseComponents, FormAnswerDatabase, FormMessageDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> MessageRepository for Repository<Client> {
    #[tracing::instrument(skip(self))]
    async fn post_message(&self, message: &Message) -> Result<(), Error> {
        self.client
            .form_message()
            .post_message(message)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn fetch_messages_by_answer_id(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<Message>, Error> {
        let answer = self
            .client
            .form_answer()
            .get_answers(answer_id)
            .await?
            .map(TryInto::<domain::form::answer::models::AnswerEntry>::try_into)
            .transpose()?;

        match answer {
            Some(answer) => self
                .client
                .form_message()
                .fetch_messages_by_form_answer(&answer)
                .await?
                .into_iter()
                .map(TryInto::<Message>::try_into)
                .collect(),
            None => Ok(Vec::new()),
        }
    }

    #[tracing::instrument(skip(self))]
    async fn update_message_body(&self, message_id: MessageId, body: String) -> Result<(), Error> {
        self.client
            .form_message()
            .update_message_body(message_id, body)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn fetch_message(&self, message_id: &MessageId) -> Result<Option<Message>, Error> {
        self.client
            .form_message()
            .fetch_message(message_id)
            .await?
            .map(TryInto::<Message>::try_into)
            .transpose()
    }

    #[tracing::instrument(skip(self))]
    async fn delete_message(&self, message_id: MessageId) -> Result<(), Error> {
        self.client
            .form_message()
            .delete_message(message_id)
            .await
            .map_err(Into::into)
    }
}
