use common::config::FRONTEND;
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
        },
        notification_repository::NotificationRepository,
        user_repository::UserRepository,
    },
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard_with_context::{Create, Read},
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
> {
    pub notification_repository: &'a NotificationRepo,
    pub active_form_repository: &'a FormRepo,
    pub user_repository: &'a UserRepo,
    pub answer_entry_set_repository: &'a AnswerEntrySetRepo,
}

impl<
    R1: NotificationRepository,
    R2: ActiveFormRepository,
    R3: UserRepository,
    R4: AnswerEntrySetRepository,
> MessageUseCase<'_, R1, R2, R3, R4>
{
    async fn read_answer_entry_set_guard_and_entry(
        &self,
        actor: &Actor,
        form_id: FormId,
        answer_id: AnswerId,
    ) -> Result<(AuthorizationGuard<AnswerEntrySet, Read>, AnswerEntry), Error> {
        let form_guard = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?;
        let form = form_guard.try_read(actor)?;

        let set_guard = self
            .answer_entry_set_repository
            .get(*form.answer_entry_set_id())
            .await?
            .ok_or(FormNotFound)?;
        let answer_entry_set = set_guard.try_read(actor)?;

        let entry = answer_entry_set
            .read_entry(answer_id, actor)
            .cloned()
            .map_err(|error| match error {
                DomainError::NotFound => Error::from(AnswerNotFound),
                error => Error::from(error),
            })?;

        Ok((set_guard, entry))
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
        let (set_guard, form_answer) = self
            .read_answer_entry_set_guard_and_entry(&actor_user, form_id, answer_id)
            .await?;

        let form_id = *form_answer.form_id();
        let answer_id = *form_answer.id();

        match Message::try_new(answer_id, *actor.id(), message_body) {
            Ok(message) => {
                if !message.can_create_for_answer(&actor_user, &form_answer) {
                    return Err(Error::from(DomainError::Forbidden));
                }

                let notification_recipient_id = form_answer
                    .author()
                    .authenticated_user_id()
                    .ok_or(Error::from(UserNotFound))?;

                let message_sender_id = *message.sender_id();

                let post_result = self
                    .answer_entry_set_repository
                    .add_message(&set_guard, answer_id, &message, &actor_user)
                    .await;

                match post_result {
                    Ok(_) if message_sender_id != notification_recipient_id => {
                        let fetched_notification_preference = self
                            .notification_repository
                            .fetch_notification_settings(notification_recipient_id.into_inner())
                            .await?;

                        let notification_preference = match fetched_notification_preference {
                            Some(settings) => settings.try_into_read(&Actor::System)?,
                            None => {
                                let recipient = self
                                    .user_repository
                                    .find_by(notification_recipient_id.into_inner())
                                    .await?
                                    .ok_or(Error::from(UserNotFound))?
                                    .try_into_read(&actor_user)?;

                                let settings: AuthorizationGuard<_, Create> =
                                    NotificationPreference::new(*recipient.id()).into();

                                self.notification_repository
                                    .create_notification_settings(&recipient, &settings)
                                    .await?;

                                settings.into_read().try_into_read(&Actor::System)?
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
        let (_set_guard, form_answer) = self
            .read_answer_entry_set_guard_and_entry(&actor_user, form_id, answer_id)
            .await?;

        let messages = form_answer
            .messages()
            .iter()
            .filter(|msg| msg.can_read_for_answer(&actor_user, &form_answer))
            .cloned()
            .collect::<Vec<_>>();

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
        let (set_guard, form_answer) = self
            .read_answer_entry_set_guard_and_entry(&actor_user, form_id, answer_id)
            .await?;

        let message = form_answer
            .find_message(*message_id)
            .ok_or(MessageNotFound)?;

        if !message.can_update_message(&actor_user) {
            return Err(Error::from(DomainError::Forbidden));
        }

        if let Some(body) = body {
            self.answer_entry_set_repository
                .update_message_body(&set_guard, answer_id, *message_id, body, &actor_user)
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
        let (set_guard, form_answer) = self
            .read_answer_entry_set_guard_and_entry(&actor_user, form_id, answer_id)
            .await?;

        let message = form_answer
            .find_message(*message_id)
            .ok_or(MessageNotFound)?;

        if !message.can_delete_message(&actor_user) {
            return Err(Error::from(DomainError::Forbidden));
        }

        self.answer_entry_set_repository
            .delete_message(&set_guard, answer_id, *message_id, &actor_user)
            .await
    }
}
