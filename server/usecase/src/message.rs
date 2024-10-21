use domain::{
    form::models::AnswerId,
    message::models::Message,
    repository::{form_repository::FormRepository, message_repository::MessageRepository},
    types::authorization_guard::{AuthorizationGuard, Read},
};
use errors::{usecase::UseCaseError, Error};

pub struct MessageUseCase<'a, MessageRepo: MessageRepository, FormRepo: FormRepository> {
    pub message_repository: &'a MessageRepo,
    pub form_repository: &'a FormRepo,
}

impl<MR: MessageRepository, FR: FormRepository> MessageUseCase<'_, MR, FR> {
    pub async fn post_message(&self, message: &Message) -> Result<(), Error> {
        self.message_repository.post_message(message).await
    }

    pub async fn get_message(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<AuthorizationGuard<Message, Read>>, Error> {
        let answers = self
            .form_repository
            .get_answers(answer_id)
            .await?
            .ok_or(UseCaseError::AnswerNotFound)?;

        self.message_repository
            .fetch_messages_by_answer_id(&answers)
            .await
    }
}
