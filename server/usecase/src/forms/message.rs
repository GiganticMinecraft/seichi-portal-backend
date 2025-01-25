use domain::form::answer::service::AnswerEntryAuthorizationContext;
use domain::repository::form::form_repository::FormRepository;
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
use errors::usecase::UseCaseError::FormNotFound;
use errors::{
    usecase::UseCaseError::{AnswerNotFound, MessageNotFound},
    Error,
};

pub struct MessageUseCase<
    'a,
    MessageRepo: MessageRepository,
    AnswerRepo: AnswerRepository,
    NotificationRepo: NotificationRepository,
    FormRepo: FormRepository,
> {
    pub message_repository: &'a MessageRepo,
    pub answer_repository: &'a AnswerRepo,
    pub notification_repository: &'a NotificationRepo,
    pub form_repository: &'a FormRepo,
}

impl<
        R1: MessageRepository,
        R2: AnswerRepository,
        R3: NotificationRepository,
        R4: FormRepository,
    > MessageUseCase<'_, R1, R2, R3, R4>
{
    pub async fn post_message(
        &self,
        actor: &User,
        message_body: String,
        answer_id: AnswerId,
    ) -> Result<(), Error> {
        let form_answer = self
            .answer_repository
            .get_answer(answer_id)
            .await?
            .ok_or(Error::from(AnswerNotFound))?
            .try_into_read_with_context_fn(&actor, move |entry| {
                let form_id = entry.form_id().to_owned();

                async move {
                    let guard = self
                        .form_repository
                        .get(form_id)
                        .await?
                        .ok_or(FormNotFound)?;

                    let form = guard.try_read(actor)?;
                    let form_settings = form.settings();

                    Ok(AnswerEntryAuthorizationContext {
                        form_visibility: form_settings.visibility().to_owned(),
                        response_period: form_settings
                            .answer_settings()
                            .response_period()
                            .to_owned(),
                        answer_visibility: form_settings.answer_settings().visibility().to_owned(),
                    })
                }
            })
            .await?;

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
        actor: &User,
        answer_id: AnswerId,
    ) -> Result<Vec<AuthorizationGuard<Message, Read>>, Error> {
        let answers = self
            .answer_repository
            .get_answer(answer_id)
            .await?
            .ok_or(Error::from(AnswerNotFound))?
            .try_into_read_with_context_fn(&actor, move |entry| {
                let form_id = entry.form_id().to_owned();

                async move {
                    let guard = self
                        .form_repository
                        .get(form_id)
                        .await?
                        .ok_or(FormNotFound)?;

                    let form = guard.try_read(&actor)?;
                    let form_settings = form.settings();

                    Ok(AnswerEntryAuthorizationContext {
                        form_visibility: form_settings.visibility().to_owned(),
                        response_period: form_settings
                            .answer_settings()
                            .response_period()
                            .to_owned(),
                        answer_visibility: form_settings.answer_settings().visibility().to_owned(),
                    })
                }
            })
            .await?;

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
