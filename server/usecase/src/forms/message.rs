use common::config::FRONTEND;
use domain::form::message_thread::models::MessageThread;
use domain::form::models::FormId;
use domain::notification::models::{NotificationContent, NotificationType};
use domain::notification::notificator::Notificator;
use domain::{
    form::{
        answer::models::{AnswerEntry, AnswerId},
        answer_entry_set::models::AnswerEntrySet,
        message::models::{Message, MessageId},
    },
    notification::models::NotificationPreference,
    repository::{
        form::{
            active_form_repository::ActiveFormRepository,
            answer_entry_set_repository::AnswerEntrySetRepository,
            message_thread_repository::MessageThreadRepository,
        },
        notification_repository::NotificationRepository,
        user_repository::UserRepository,
    },
    types::{
        authorization_guard::{Allowed, AuthorizationGuard},
        authorization_guard::{Create, Read, Update},
    },
    user::models::{ActiveUser, Actor},
};
use errors::{
    Error,
    domain::DomainError,
    usecase::UseCaseError::{AnswerNotFound, FormNotFound, MessageNotFound, UserNotFound},
};

use crate::{models::MessageWithSender, user_reference_resolver::resolve_user_references};

pub struct MessageUseCase<
    'a,
    NotificationRepo: NotificationRepository,
    FormRepo: ActiveFormRepository,
    UserRepo: UserRepository,
    AnswerEntrySetRepo: AnswerEntrySetRepository,
    MessageThreadRepo: MessageThreadRepository,
> {
    pub notification_repository: &'a NotificationRepo,
    pub active_form_repository: &'a FormRepo,
    pub user_repository: &'a UserRepo,
    pub answer_entry_set_repository: &'a AnswerEntrySetRepo,
    pub message_thread_repository: &'a MessageThreadRepo,
}

impl<
    R1: NotificationRepository,
    R2: ActiveFormRepository,
    R3: UserRepository,
    R4: AnswerEntrySetRepository,
    R5: MessageThreadRepository,
> MessageUseCase<'_, R1, R2, R3, R4, R5>
{
    async fn read_answer_entry_set_guard_and_entry(
        &self,
        actor: &Actor,
        form_id: FormId,
        answer_id: AnswerId,
    ) -> Result<(Allowed<AnswerEntrySet, Read>, Allowed<AnswerEntry, Read>), Error> {
        let form_guard = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?;
        let form = form_guard.try_read(actor.clone())?;

        let set_guard = self
            .answer_entry_set_repository
            .get(*form.answer_entry_set_id())
            .await?
            .ok_or(FormNotFound)?;
        let answer_entry_set = set_guard.try_read(actor.clone())?;

        let entry = answer_entry_set
            .read_entry(answer_id)
            .map_err(|error| match error {
                DomainError::NotFound => Error::from(AnswerNotFound),
                error => Error::from(error),
            })?;

        Ok((answer_entry_set, entry))
    }

    pub async fn post_message<N: Notificator>(
        &self,
        actor: &ActiveUser,
        form_id: FormId,
        message_body: String,
        answer_id: AnswerId,
        notificator: &N,
    ) -> Result<(), Error> {
        let actor_user = Actor::from(actor.clone());
        let (_set_guard, form_answer) = self
            .read_answer_entry_set_guard_and_entry(&actor_user, form_id, answer_id)
            .await?;

        match Message::try_new(*actor.id(), message_body) {
            Ok(message) => {
                let notification_recipient_id = form_answer
                    .author()
                    .authenticated_user_id()
                    .ok_or(Error::from(UserNotFound))?;

                let message_sender_id = *message.sender_id();

                let post_result = match self
                    .message_thread_repository
                    .get_by_answer_id(answer_id)
                    .await?
                {
                    Some(thread_guard) => {
                        let thread = thread_guard.try_read(actor_user.clone())?;
                        let updated = thread.value().clone().add_message(message);
                        let guard = AuthorizationGuard::<MessageThread, Update>::from(updated);
                        self.message_thread_repository
                            .update(guard.try_update(actor_user.clone())?)
                            .await
                    }
                    None => {
                        let answer_author_id = form_answer
                            .author()
                            .authenticated_user_id()
                            .ok_or(Error::from(UserNotFound))?;
                        let thread =
                            MessageThread::new(answer_id, answer_author_id).add_message(message);
                        let guard = AuthorizationGuard::<MessageThread, Create>::from(thread);
                        self.message_thread_repository
                            .create(guard.try_create(actor_user.clone())?)
                            .await
                    }
                };

                match post_result {
                    Ok(_) if message_sender_id != notification_recipient_id => {
                        let fetched_notification_preference = self
                            .notification_repository
                            .fetch_notification_settings(notification_recipient_id.into_inner())
                            .await?;

                        let notification_preference = match fetched_notification_preference {
                            Some(settings) => settings.try_read(Actor::System)?.into_inner(),
                            None => {
                                let recipient = self
                                    .user_repository
                                    .find_by(notification_recipient_id.into_inner())
                                    .await?
                                    .ok_or(Error::from(UserNotFound))?
                                    .try_read(actor_user.clone())?
                                    .into_inner();

                                let preference = NotificationPreference::new(*recipient.id());

                                self.notification_repository
                                    .create_notification_settings(
                                        AuthorizationGuard::<_, Create>::from(preference.clone())
                                            .try_create(Actor::from(recipient.clone()))?,
                                    )
                                    .await?;

                                AuthorizationGuard::<_, Read>::from(preference)
                                    .try_read(Actor::System)?
                                    .into_inner()
                            }
                        };

                        let url = &*FRONTEND.url;
                        notificator
                            .notify(
                                notification_recipient_id,
                                NotificationType::MessageReceived,
                                &notification_preference,
                                &NotificationContent::new(vec![
                                    "あなたの回答にメッセージが送信されました。".to_string(),
                                    "メッセージを確認してください。".to_string(),
                                    format!("{url}/forms/{form_id}/answers/{answer_id}/messages"),
                                ]),
                            )
                            .await?;

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
        actor: &ActiveUser,
        form_id: FormId,
        answer_id: AnswerId,
    ) -> Result<Vec<MessageWithSender>, Error> {
        let actor_user = Actor::from(actor.clone());
        self.read_answer_entry_set_guard_and_entry(&actor_user, form_id, answer_id)
            .await?;

        let messages = match self
            .message_thread_repository
            .get_by_answer_id(answer_id)
            .await?
        {
            None => vec![],
            Some(thread_guard) => {
                let thread = thread_guard.try_read(actor_user.clone())?;
                thread.messages().to_vec()
            }
        };

        let sender_ids = messages.iter().map(|m| *m.sender_id()).collect();
        let senders = resolve_user_references(self.user_repository, actor, sender_ids).await?;

        messages
            .into_iter()
            .map(|message| {
                let sender = senders
                    .get(message.sender_id())
                    .cloned()
                    .ok_or(Error::from(UserNotFound))?;
                Ok(MessageWithSender { message, sender })
            })
            .collect()
    }

    pub async fn update_message_body(
        &self,
        actor: &ActiveUser,
        form_id: FormId,
        answer_id: AnswerId,
        message_id: &MessageId,
        body: Option<String>,
    ) -> Result<(), Error> {
        let actor_user = Actor::from(actor.clone());
        self.read_answer_entry_set_guard_and_entry(&actor_user, form_id, answer_id)
            .await?;

        let thread_guard = self
            .message_thread_repository
            .get_by_answer_id(answer_id)
            .await?
            .ok_or(Error::from(MessageNotFound))?;

        let thread = thread_guard.try_read(actor_user.clone())?;

        if let Some(body) = body {
            let updated =
                thread
                    .value()
                    .clone()
                    .update_message_body(*message_id, &actor_user, body)?;
            let guard = AuthorizationGuard::<MessageThread, Update>::from(updated);
            self.message_thread_repository
                .update(guard.try_update(actor_user.clone())?)
                .await?;
        }

        Ok(())
    }

    pub async fn delete_message(
        &self,
        actor: &ActiveUser,
        form_id: FormId,
        answer_id: AnswerId,
        message_id: &MessageId,
    ) -> Result<(), Error> {
        let actor_user = Actor::from(actor.clone());
        self.read_answer_entry_set_guard_and_entry(&actor_user, form_id, answer_id)
            .await?;

        let thread_guard = self
            .message_thread_repository
            .get_by_answer_id(answer_id)
            .await?
            .ok_or(Error::from(MessageNotFound))?;

        let thread = thread_guard.try_read(actor_user.clone())?;

        let updated = thread
            .value()
            .clone()
            .remove_message(*message_id, &actor_user)?;
        let guard = AuthorizationGuard::<MessageThread, Update>::from(updated);
        self.message_thread_repository
            .update(guard.try_update(actor_user.clone())?)
            .await
    }
}
