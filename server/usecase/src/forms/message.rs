use domain::{
    form::{
        answer::{models::AnswerId, service::AnswerEntryAuthorizationContext},
        message::{
            models::{Message, MessageId},
            service::MessageAuthorizationContext,
        },
    },
    notification::models::{
        DiscordDMNotification, DiscordDMNotificationType, NotificationSettings,
    },
    repository::{
        form::{
            answer_repository::AnswerRepository, form_repository::FormRepository,
            message_repository::MessageRepository,
        },
        notification_repository::NotificationRepository,
        user_repository::UserRepository,
    },
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard_with_context::{AuthorizationGuardWithContext, Create},
    },
    user::models::User,
};
use errors::{
    Error,
    usecase::UseCaseError::{AnswerNotFound, FormNotFound, MessageNotFound},
};
use resource::outgoing::connection::ConnectionPool;

pub struct MessageUseCase<
    'a,
    MessageRepo: MessageRepository,
    AnswerRepo: AnswerRepository,
    NotificationRepo: NotificationRepository,
    FormRepo: FormRepository,
    UserRepo: UserRepository,
> {
    pub message_repository: &'a MessageRepo,
    pub answer_repository: &'a AnswerRepo,
    pub notification_repository: &'a NotificationRepo,
    pub form_repository: &'a FormRepo,
    pub user_repository: &'a UserRepo,
}

impl<
    R1: MessageRepository,
    R2: AnswerRepository,
    R3: NotificationRepository,
    R4: FormRepository,
    R5: UserRepository,
> MessageUseCase<'_, R1, R2, R3, R4, R5>
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
            .try_into_read_with_context_fn(actor, move |entry| {
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

        let form_id = form_answer.form_id().to_owned();
        let answer_id = form_answer.id().to_owned();

        match Message::try_new(answer_id, actor.to_owned(), message_body) {
            Ok(message) => {
                let notification_recipient = form_answer.user().to_owned();

                let message_sender = message.sender().to_owned();
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
                    Ok(_) if message_sender.id != notification_recipient.id => {
                        if let Some(discord_user) = self
                            .user_repository
                            .fetch_discord_user(actor, &notification_recipient.to_owned().into())
                            .await?
                        {
                            let settings = self
                                .notification_repository
                                .fetch_notification_settings(notification_recipient.id)
                                .await?;

                            let settings = match settings {
                                Some(settings) => settings.try_into_read(actor)?,
                                None => {
                                    let settings: AuthorizationGuard<_, Create> =
                                        NotificationSettings::new(
                                            notification_recipient.to_owned(),
                                        )
                                        .into();

                                    self.notification_repository
                                        .create_notification_settings(
                                            &notification_recipient,
                                            &settings,
                                        )
                                        .await?;

                                    settings.into_read().try_into_read(actor)?
                                }
                            };

                            DiscordDMNotification::new(ConnectionPool::new().await)
                                .send_message_notification(
                                    discord_user.id().to_owned(),
                                    &settings,
                                    DiscordDMNotificationType::Message { form_id, answer_id },
                                )
                                .await?;
                        }

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
    ) -> Result<Vec<Message>, Error> {
        let answers = self
            .answer_repository
            .get_answer(answer_id)
            .await?
            .ok_or(Error::from(AnswerNotFound))?
            .try_into_read_with_context_fn(actor, move |entry| {
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

        let message_context = MessageAuthorizationContext {
            related_answer_entry: answers,
        };

        self.message_repository
            .fetch_messages_by_answer(&message_context.related_answer_entry)
            .await?
            .into_iter()
            .map(|guard| guard.try_into_read(actor, &message_context))
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub async fn update_message_body(
        &self,
        actor: &User,
        answer_id: AnswerId,
        message_id: &MessageId,
        body: String,
    ) -> Result<(), Error> {
        let message = self
            .message_repository
            .fetch_message(message_id)
            .await?
            .ok_or(MessageNotFound)?;

        let answer_entry = self
            .answer_repository
            .get_answer(answer_id)
            .await?
            .ok_or(Error::from(AnswerNotFound))?
            .try_into_read_with_context_fn(actor, move |entry| {
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

        let message_context = MessageAuthorizationContext {
            related_answer_entry: answer_entry,
        };

        if *message
            .try_read(actor, &message_context)?
            .related_answer_id()
            != answer_id
        {
            return Err(Error::from(MessageNotFound));
        }

        self.message_repository
            .update_message_body(actor, &message_context, message.into_update(), body)
            .await
    }

    pub async fn delete_message(
        &self,
        actor: &User,
        answer_id: AnswerId,
        message_id: &MessageId,
    ) -> Result<(), Error> {
        let message = self
            .message_repository
            .fetch_message(message_id)
            .await?
            .ok_or(MessageNotFound)?;

        let answer_entry = self
            .answer_repository
            .get_answer(answer_id)
            .await?
            .ok_or(Error::from(AnswerNotFound))?
            .try_into_read_with_context_fn(actor, move |entry| {
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

        let message_context = MessageAuthorizationContext {
            related_answer_entry: answer_entry,
        };

        if *(&message
            .try_read(actor, &message_context)?
            .related_answer_id())
            .to_owned()
            != answer_id
        {
            return Err(Error::from(MessageNotFound));
        }

        self.message_repository
            .delete_message(actor, &message_context, message.into_delete())
            .await
    }
}
