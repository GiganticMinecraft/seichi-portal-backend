use domain::{message::models::Message, repository::message_repository::MessageRepository};
use errors::Error;

pub struct MessageUseCase<'a, MessageRepo: MessageRepository> {
    pub repository: &'a MessageRepo,
}

impl<R: MessageRepository> MessageUseCase<'_, R> {
    pub async fn post_message(&self, message: &Message) -> Result<(), Error> {
        self.repository.post_message(message).await
    }
}
