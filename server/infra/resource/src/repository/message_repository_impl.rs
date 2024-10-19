use async_trait::async_trait;
use domain::{
    form::models::PostedAnswers, message::models::Message,
    repository::message_repository::MessageRepository,
};
use errors::Error;

use crate::{
    database::components::{DatabaseComponents, MessageDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> MessageRepository for Repository<Client> {
    async fn post_message(&self, message: &Message) -> Result<(), Error> {
        self.client
            .message()
            .post_message(message)
            .await
            .map_err(Into::into)
    }

    async fn fetch_messages_by_answer_id(
        &self,
        answers: &PostedAnswers,
    ) -> Result<Vec<Message>, Error> {
        self.client
            .message()
            .fetch_messages_by_answer_id(answers)
            .await?
            .into_iter()
            .map(|dto| Ok(dto.try_into()?))
            .collect::<Result<Vec<_>, _>>()
    }
}
