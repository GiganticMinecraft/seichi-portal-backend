use common::config::FRONTEND;
use domain::form::models::FormId;
use domain::notification::models::{NotificationContent, NotificationType};
use domain::notification::notificator::Notificator;
use domain::{
    form::{
        answer::{models::AnswerId, service::AnswerEntryAuthorizationContext},
        message::{
            models::{Message, MessageId},
            service::MessageAuthorizationContext,
        },
    },
    notification::models::NotificationPreference,
    repository::{
        form::{
            active_form_repository::ActiveFormRepository, answer_repository::AnswerRepository,
            message_repository::MessageRepository,
        },
        notification_repository::NotificationRepository,
        user_repository::UserRepository,
    },
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard_with_context::{AuthorizationGuardWithContext, Create},
    },
    user::models::{ActiveUser, User},
};
use errors::{
    Error,
    usecase::UseCaseError::{AnswerNotFound, FormNotFound, MessageNotFound, UserNotFound},
};

use crate::{models::MessageWithSender, user_reference_resolver::resolve_user_references};

pub struct MessageUseCase<
    'a,
    MessageRepo: MessageRepository,
    AnswerRepo: AnswerRepository,
    NotificationRepo: NotificationRepository,
    FormRepo: ActiveFormRepository,
    UserRepo: UserRepository,
> {
    pub message_repository: &'a MessageRepo,
    pub answer_repository: &'a AnswerRepo,
    pub notification_repository: &'a NotificationRepo,
    pub active_form_repository: &'a FormRepo,
    pub user_repository: &'a UserRepo,
}

impl<
    R1: MessageRepository,
    R2: AnswerRepository,
    R3: NotificationRepository,
    R4: ActiveFormRepository,
    R5: UserRepository,
> MessageUseCase<'_, R1, R2, R3, R4, R5>
{
    pub async fn post_message<N: Notificator>(
        &self,
        actor: &ActiveUser,
        form_id: FormId,
        message_body: String,
        answer_id: AnswerId,
        notificator: &N,
    ) -> Result<(), Error> {
        let actor_user = User::from(actor.clone());
        let form_guard = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?;

        let form = form_guard.try_read(&actor_user)?;
        let form_settings = form.settings();

        let answer_entry_authorization_context = AnswerEntryAuthorizationContext {
            form_visibility: form_settings.visibility().to_owned(),
            response_period: form_settings.answer_settings().response_period().to_owned(),
            answer_visibility: form_settings.answer_settings().visibility().to_owned(),
            allow_temporary_answers: form_settings.allow_temporary_answers(),
        };
        let form_answer = self
            .answer_repository
            .get_answer(answer_id)
            .await?
            .ok_or(Error::from(AnswerNotFound))?
            .try_into_read(&actor_user, &answer_entry_authorization_context)?;

        let form_id = form_answer.form_id().to_owned();
        let answer_id = form_answer.id().to_owned();

        match Message::try_new(answer_id, *actor.id(), message_body) {
            Ok(message) => {
                let notification_recipient_id = form_answer
                    .author()
                    .authenticated_user_id()
                    .ok_or(Error::from(UserNotFound))?;

                let message_sender_id = *message.sender_id();
                let message_context = MessageAuthorizationContext {
                    related_answer_entry: form_answer,
                };

                let post_message_result = self
                    .message_repository
                    .post_message(
                        actor,
                        &message_context,
                        AuthorizationGuardWithContext::new(message),
                    )
                    .await;

                match post_message_result {
                    Ok(_) if message_sender_id != notification_recipient_id => {
                        let fetched_notification_preference = self
                            .notification_repository
                            .fetch_notification_settings(notification_recipient_id.into_inner())
                            .await?;

                        // SAFETY: 通知設定の読み取りはシステム的な処理であり、メッセージ送信者が
                        // 受信者の通知設定を読める必要がある。適切なシステム権限の仕組みは別途対応する。
                        let notification_preference = match fetched_notification_preference {
                            Some(settings) => unsafe { settings.into_read_unchecked() },
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

                                unsafe { settings.into_read().into_read_unchecked() }
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
        let actor_user = User::from(actor.clone());
        let form_guard = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?;

        let form = form_guard.try_read(&actor_user)?;
        let form_settings = form.settings();
        let answer_entry_authorization_context = AnswerEntryAuthorizationContext {
            form_visibility: form_settings.visibility().to_owned(),
            response_period: form_settings.answer_settings().response_period().to_owned(),
            answer_visibility: form_settings.answer_settings().visibility().to_owned(),
            allow_temporary_answers: form_settings.allow_temporary_answers(),
        };
        let answers = self
            .answer_repository
            .get_answer(answer_id)
            .await?
            .ok_or(Error::from(AnswerNotFound))?
            .try_into_read(&actor_user, &answer_entry_authorization_context)?;

        let message_context = MessageAuthorizationContext {
            related_answer_entry: answers,
        };

        let messages = self
            .message_repository
            .fetch_messages_by_answer(&message_context.related_answer_entry)
            .await?
            .into_iter()
            .map(|guard| guard.try_into_read(&actor_user, &message_context))
            .collect::<Result<Vec<_>, _>>()
            .map_err(Error::from)?;

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
        let actor_user = User::from(actor.clone());
        let message = self
            .message_repository
            .fetch_message(message_id)
            .await?
            .ok_or(MessageNotFound)?;

        let form_guard = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?;
        let form = form_guard.try_read(&actor_user)?;
        let form_settings = form.settings();

        let answer_entry_authorization_context = AnswerEntryAuthorizationContext {
            form_visibility: form_settings.visibility().to_owned(),
            response_period: form_settings.answer_settings().response_period().to_owned(),
            answer_visibility: form_settings.answer_settings().visibility().to_owned(),
            allow_temporary_answers: form_settings.allow_temporary_answers(),
        };
        let answer_entry = self
            .answer_repository
            .get_answer(answer_id)
            .await?
            .ok_or(Error::from(AnswerNotFound))?
            .try_into_read(&actor_user, &answer_entry_authorization_context)?;

        let message_context = MessageAuthorizationContext {
            related_answer_entry: answer_entry,
        };

        if *message
            .try_read(&actor_user, &message_context)?
            .related_answer_id()
            != answer_id
        {
            return Err(Error::from(MessageNotFound));
        }

        if let Some(body) = body {
            self.message_repository
                .update_message_body(actor, &message_context, message.into_update(), body)
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
        let actor_user = User::from(actor.clone());
        let message = self
            .message_repository
            .fetch_message(message_id)
            .await?
            .ok_or(MessageNotFound)?;

        let form_guard = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?;
        let form = form_guard.try_read(&actor_user)?;
        let form_settings = form.settings();

        let answer_entry_authorization_context = AnswerEntryAuthorizationContext {
            form_visibility: form_settings.visibility().to_owned(),
            response_period: form_settings.answer_settings().response_period().to_owned(),
            answer_visibility: form_settings.answer_settings().visibility().to_owned(),
            allow_temporary_answers: form_settings.allow_temporary_answers(),
        };
        let answer_entry = self
            .answer_repository
            .get_answer(answer_id)
            .await?
            .ok_or(Error::from(AnswerNotFound))?
            .try_into_read(&actor_user, &answer_entry_authorization_context)?;

        let message_context = MessageAuthorizationContext {
            related_answer_entry: answer_entry,
        };

        if *message
            .try_read(&actor_user, &message_context)?
            .related_answer_id()
            != answer_id
        {
            return Err(Error::from(MessageNotFound));
        }

        self.message_repository
            .delete_message(actor, &message_context, message.into_delete())
            .await
    }
}
