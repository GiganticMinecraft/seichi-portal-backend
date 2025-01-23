use domain::types::authorization_guard_with_context::Read;
use domain::{
    form::{
        answer::models::AnswerId,
        message::models::{Message, MessageId},
    },
    notification::models::{Notification, NotificationSource},
    repository::{
        form::{answer_repository::AnswerRepository, message_repository::MessageRepository},
        notification_repository::NotificationRepository,
    },
    types::authorization_guard::AuthorizationGuard,
    user::models::User,
};
use errors::{
    usecase::UseCaseError::{AnswerNotFound, MessageNotFound},
    Error,
};

pub struct MessageUseCase<
    'a,
    MessageRepo: MessageRepository,
    AnswerRepo: AnswerRepository,
    NotificationRepo: NotificationRepository,
> {
    pub message_repository: &'a MessageRepo,
    pub answer_repository: &'a AnswerRepo,
    pub notification_repository: &'a NotificationRepo,
}

impl<R1: MessageRepository, R2: AnswerRepository, R3: NotificationRepository>
    MessageUseCase<'_, R1, R2, R3>
{
    pub async fn post_message(
        &self,
        actor: User,
        message_body: String,
        answer_id: AnswerId,
    ) -> Result<(), Error> {
        let form_answer = match self.answer_repository.get_answers(answer_id).await? {
            Some(form_answer) => form_answer,
            None => return Err(Error::from(AnswerNotFound)),
        };

        match Message::try_new(form_answer, actor.to_owned(), message_body) {
            Ok(message) => {
                let notification = Notification::new(
                    NotificationSource::Message(message.id().to_owned()),
                    message.related_answer().user().to_owned(),
                );

                let message_sender = message.sender().to_owned();

                let post_message_result = self
                    .message_repository
                    .post_message(&actor, message.into())
                    .await;

                match post_message_result {
                    Ok(_) if message_sender.id != notification.recipient().id => {
                        self.notification_repository.create(&notification).await?;
                        Ok(())
                    }
                    Err(error) => Err(error),
                    _ => Ok(()),
                }
            }
            Err(error) => Err(Error::from(error)),
        }
    }

    pub async fn get_messages(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<AuthorizationGuard<Message, Read>>, Error> {
        let answers = self
            .answer_repository
            .get_answers(answer_id)
            .await?
            .ok_or(AnswerNotFound)?;

        self.message_repository
            .fetch_messages_by_answer(&answers)
            .await
    }

    pub async fn update_message_body(
        &self,
        actor: &User,
        answer_id: &AnswerId,
        message_id: &MessageId,
        body: String,
    ) -> Result<(), Error> {
        let message = self
            .message_repository
            .fetch_message(message_id)
            .await?
            .ok_or(MessageNotFound)?;

        if &message.try_read(actor)?.related_answer().id().to_owned() != answer_id {
            return Err(Error::from(MessageNotFound));
        }

        self.message_repository
            .update_message_body(actor, message.into_update(), body)
            .await
    }

    pub async fn delete_message(
        &self,
        actor: &User,
        answer_id: &AnswerId,
        message_id: &MessageId,
    ) -> Result<(), Error> {
        let message = self
            .message_repository
            .fetch_message(message_id)
            .await?
            .ok_or(MessageNotFound)?;

        if &message.try_read(actor)?.related_answer().id().to_owned() != answer_id {
            return Err(Error::from(MessageNotFound));
        }

        self.message_repository
            .delete_message(actor, message.into_delete())
            .await
    }
}
