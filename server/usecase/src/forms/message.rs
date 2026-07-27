use chrono::Utc;
use common::config::FRONTEND;
use domain::form::models::FormId;
use domain::notification::models::{NotificationContent, NotificationType};
use domain::notification::notificator::Notificator;
use domain::{
    account::models::AccountUser,
    auth::Actor,
    form::{
        answer::{AnswerEntry, AnswerId},
        message::{
            Message, MessageBody, MessageHistoryEntry, MessageHistoryPagePosition, MessageId,
        },
    },
    notification::models::NotificationPreference,
    pagination::{Page, PageRequest},
    repository::{
        form::{
            active_form_repository::ActiveFormRepository,
            answer_entry_repository::AnswerEntryRepository,
            message_thread_repository::MessageThreadRepository,
        },
        notification_repository::NotificationRepository,
        user_repository::UserRepository,
    },
    types::{
        authorization_guard::{Allowed, AuthorizationGuard},
        authorization_guard::{Create, Read},
    },
};
use errors::{
    Error,
    usecase::UseCaseError::{AnswerNotFound, FormNotFound, MessageNotFound, UserNotFound},
};

use crate::{
    application_event::{ApplicationActor, ApplicationEvent, ApplicationEventPublisher},
    models::MessageWithSender,
    user_reference_resolver::resolve_user_references,
};

pub struct MessageUseCase<
    'a,
    NotificationRepo: NotificationRepository,
    FormRepo: ActiveFormRepository,
    UserRepo: UserRepository,
    AnswerEntryRepo: AnswerEntryRepository,
    MessageThreadRepo: MessageThreadRepository,
> {
    pub notification_repository: &'a NotificationRepo,
    pub active_form_repository: &'a FormRepo,
    pub user_repository: &'a UserRepo,
    pub answer_entry_repository: &'a AnswerEntryRepo,
    pub message_thread_repository: &'a MessageThreadRepo,
    pub application_event_publisher: Option<&'a dyn ApplicationEventPublisher>,
}

impl<
    R1: NotificationRepository,
    R2: ActiveFormRepository,
    R3: UserRepository,
    R4: AnswerEntryRepository,
    R5: MessageThreadRepository,
> MessageUseCase<'_, R1, R2, R3, R4, R5>
{
    async fn read_answer_entry(
        &self,
        actor: &Actor,
        form_id: FormId,
        answer_id: AnswerId,
    ) -> Result<Allowed<AnswerEntry, Read>, Error> {
        let form = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?
            .try_read(actor.clone())?;

        self.answer_entry_repository
            .get(&form, answer_id)
            .await?
            .ok_or(AnswerNotFound)
            .map_err(Into::into)
    }

    pub async fn post_message<N: Notificator>(
        &self,
        actor: &AccountUser,
        form_id: FormId,
        message_body: MessageBody,
        answer_id: AnswerId,
        notificator: &N,
    ) -> Result<(), Error> {
        let actor_user = Actor::from(actor.clone());
        let form_answer = self
            .read_answer_entry(&actor_user, form_id, answer_id)
            .await?;

        let message = Message::new(*actor.id(), message_body);
        let message_id = message.id().to_string();
        let message_body = message.body().as_str().to_owned();
        let message_sender_id = *message.sender_id();

        let thread = self
            .message_thread_repository
            .get_for_answer(&form_answer)
            .await?
            .try_into_update()?;
        let post = thread.try_post_message(message)?;
        let notification_recipient_id = *post.answer_author_id();
        self.message_thread_repository.append(post).await?;
        if let Some(publisher) = self.application_event_publisher {
            publisher.publish(ApplicationEvent::MessageCreated {
                actor: ApplicationActor::from(actor),
                form_id: form_id.to_string(),
                answer_id: answer_id.to_string(),
                message_id,
                body: message_body,
            });
        }

        if message_sender_id != notification_recipient_id {
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
        }

        Ok(())
    }

    pub async fn get_messages(
        &self,
        actor: &AccountUser,
        form_id: FormId,
        answer_id: AnswerId,
    ) -> Result<Vec<MessageWithSender>, Error> {
        let actor_user = Actor::from(actor.clone());
        let form_answer = self
            .read_answer_entry(&actor_user, form_id, answer_id)
            .await?;

        let messages = self
            .message_thread_repository
            .get_for_answer(&form_answer)
            .await?
            .messages()
            .to_vec();

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
        actor: &AccountUser,
        form_id: FormId,
        answer_id: AnswerId,
        message_id: &MessageId,
        body: Option<MessageBody>,
    ) -> Result<(), Error> {
        let actor_user = Actor::from(actor.clone());
        let form_answer = self
            .read_answer_entry(&actor_user, form_id, answer_id)
            .await?;

        if let Some(body) = body {
            let thread = self
                .message_thread_repository
                .get_for_answer(&form_answer)
                .await?
                .try_into_update()?;

            let current = thread
                .find_message(*message_id)
                .ok_or(Error::from(MessageNotFound))?;
            if current.body() == &body {
                return Ok(());
            }
            let body_for_event = body.as_str().to_owned();

            let updated = thread.authorize_message_update(*message_id, body)?;
            self.message_thread_repository
                .update_message(updated, Utc::now())
                .await?;
            if let Some(publisher) = self.application_event_publisher {
                publisher.publish(ApplicationEvent::MessageUpdated {
                    actor: ApplicationActor::from(actor),
                    form_id: form_id.to_string(),
                    answer_id: answer_id.to_string(),
                    message_id: message_id.to_string(),
                    body: body_for_event,
                });
            }
        }

        Ok(())
    }

    pub async fn delete_message(
        &self,
        actor: &AccountUser,
        form_id: FormId,
        answer_id: AnswerId,
        message_id: &MessageId,
    ) -> Result<(), Error> {
        let actor_user = Actor::from(actor.clone());
        let form_answer = self
            .read_answer_entry(&actor_user, form_id, answer_id)
            .await?;

        let thread = self
            .message_thread_repository
            .get_for_answer(&form_answer)
            .await?
            .try_into_update()?;

        let message_body = thread
            .find_message(*message_id)
            .ok_or(Error::from(MessageNotFound))?
            .body()
            .as_str()
            .to_owned();

        let message = thread.authorize_message_delete(*message_id, Utc::now())?;
        self.message_thread_repository
            .delete_message(message)
            .await?;
        if let Some(publisher) = self.application_event_publisher {
            publisher.publish(ApplicationEvent::MessageDeleted {
                actor: ApplicationActor::from(actor),
                form_id: form_id.to_string(),
                answer_id: answer_id.to_string(),
                message_id: message_id.to_string(),
                body: message_body,
            });
        }

        Ok(())
    }

    pub async fn get_history(
        &self,
        actor: &AccountUser,
        form_id: FormId,
        answer_id: AnswerId,
        request: PageRequest<MessageHistoryPagePosition>,
    ) -> Result<Page<Allowed<MessageHistoryEntry, Read>, MessageHistoryPagePosition>, Error> {
        let actor_user = Actor::from(actor.clone());
        let form_answer = self
            .read_answer_entry(&actor_user, form_id, answer_id)
            .await?;
        let thread = self
            .message_thread_repository
            .get_for_answer(&form_answer)
            .await?;
        self.message_thread_repository
            .history(&thread, request)
            .await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    };

    use super::*;
    use async_trait::async_trait;
    use domain::{
        account::models::{AccountUser, Role, UserId},
        form::{
            answer::{AnswerAuthor, AnswerEntry, AnswerId, AnswerTitle, TemporaryAnswerAuthor},
            message::{DeletedMessage, MessageHistoryEntry, MessageId, MessagePost},
            message_thread::MessageThread,
            models::{ActiveForm, FormDescription, FormTitle, QuestionSet},
            question::Question,
        },
        notification::models::{NotificationContent, NotificationType},
        pagination::{Page, PageLimit},
        types::authorization_guard::{Create, Update},
    };
    use types::non_empty_vec::NonEmptyVec;
    use uuid::Uuid;

    use crate::{
        application_event::{ApplicationEvent, ApplicationEventPublisher},
        test_utils::repositories::{FormUseCaseTestRepositories, InMemoryAnswerEntryRepository},
    };

    #[derive(Default)]
    struct RecordingPublisher(Mutex<Vec<ApplicationEvent>>);

    impl ApplicationEventPublisher for RecordingPublisher {
        fn publish(&self, event: ApplicationEvent) {
            self.0.lock().unwrap().push(event);
        }
    }

    #[derive(Default)]
    struct InMemoryMessageThreadRepository(Mutex<Vec<(AnswerId, Message)>>);

    impl InMemoryMessageThreadRepository {
        fn with_messages(answer_id: AnswerId, messages: Vec<Message>) -> Self {
            Self(Mutex::new(
                messages
                    .into_iter()
                    .map(|message| (answer_id, message))
                    .collect(),
            ))
        }

        fn only_message_id(&self) -> MessageId {
            *self.0.lock().unwrap()[0].1.id()
        }

        fn message_count_for(&self, answer_id: AnswerId) -> usize {
            self.0
                .lock()
                .unwrap()
                .iter()
                .filter(|(stored_answer_id, _)| *stored_answer_id == answer_id)
                .count()
        }

        fn stored_messages(&self) -> Vec<Message> {
            self.0
                .lock()
                .unwrap()
                .iter()
                .map(|(_, message)| message.clone())
                .collect()
        }
    }

    #[async_trait]
    impl MessageThreadRepository for InMemoryMessageThreadRepository {
        async fn get_for_answer(
            &self,
            answer: &Allowed<AnswerEntry, Read>,
        ) -> Result<Allowed<MessageThread, Read>, Error> {
            let answer_id = *answer.id();
            let messages = self
                .0
                .lock()
                .unwrap()
                .iter()
                .filter(|(related_answer_id, _)| *related_answer_id == answer_id)
                .map(|(_, message)| message.clone())
                .collect();
            answer.message_thread(messages).map_err(Error::from)
        }

        async fn append(&self, post: Allowed<MessagePost, Create>) -> Result<(), Error> {
            let post = post.into_inner();
            let answer_id = *post.answer_id();
            self.0
                .lock()
                .unwrap()
                .push((answer_id, post.into_message()));
            Ok(())
        }

        async fn update_message(
            &self,
            message: Allowed<Message, Update>,
            _updated_at: chrono::DateTime<Utc>,
        ) -> Result<(), Error> {
            let message = message.into_inner();
            if let Some(stored_message) = self
                .0
                .lock()
                .unwrap()
                .iter_mut()
                .find(|(_, stored_message)| stored_message.id() == message.id())
            {
                stored_message.1 = message;
            }
            Ok(())
        }

        async fn delete_message(
            &self,
            message: Allowed<DeletedMessage, Create>,
        ) -> Result<(), Error> {
            let message_id = *message.message().id();
            self.0
                .lock()
                .unwrap()
                .retain(|(_, stored_message)| *stored_message.id() != message_id);
            Ok(())
        }

        async fn history(
            &self,
            _message_thread: &Allowed<MessageThread, Read>,
            _request: PageRequest<MessageHistoryPagePosition>,
        ) -> Result<Page<Allowed<MessageHistoryEntry, Read>, MessageHistoryPagePosition>, Error>
        {
            Ok(Page::new(Vec::new(), None))
        }
    }

    struct NoopNotificator;

    #[async_trait]
    impl Notificator for NoopNotificator {
        async fn notify(
            &self,
            _recipient: UserId,
            _notification_type: NotificationType,
            _notification_preference: &NotificationPreference,
            _content: &NotificationContent,
        ) -> Result<(), Error> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct FailingNotificator {
        called: AtomicBool,
        recipient: Mutex<Option<UserId>>,
    }

    #[async_trait]
    impl Notificator for FailingNotificator {
        async fn notify(
            &self,
            recipient: UserId,
            _notification_type: NotificationType,
            _notification_preference: &NotificationPreference,
            _content: &NotificationContent,
        ) -> Result<(), Error> {
            self.called.store(true, Ordering::Relaxed);
            *self.recipient.lock().unwrap() = Some(recipient);
            Err(errors::domain::DomainError::InvalidEntity {
                message: "notification failed".to_string(),
            }
            .into())
        }
    }

    fn user() -> AccountUser {
        AccountUser::new(
            "admin".to_string(),
            UserId::from(Uuid::new_v4()),
            Role::Administrator,
        )
    }

    fn form_and_answer_author(author: AnswerAuthor) -> (ActiveForm, AnswerEntry) {
        let question = Question::new_text(
            "body".to_string().try_into().unwrap(),
            0,
            "Body".to_string().try_into().unwrap(),
            None,
            false,
        )
        .unwrap();
        let form = ActiveForm::new(
            FormTitle::new("Form".to_string().try_into().unwrap()),
            FormDescription::new(String::new()),
            QuestionSet::try_new(NonEmptyVec::try_new(vec![question]).unwrap()).unwrap(),
        );
        let answer = unsafe {
            AnswerEntry::from_raw_parts(
                AnswerId::new(),
                *form.id(),
                author,
                Utc::now(),
                AnswerTitle::new(None),
                Vec::new(),
            )
        };
        (form, answer)
    }

    fn form_and_answer(user: &AccountUser) -> (ActiveForm, AnswerEntry) {
        form_and_answer_author(AnswerAuthor::AuthenticatedUser(*user.id()))
    }

    fn form_and_temporary_answer() -> (ActiveForm, AnswerEntry) {
        form_and_answer_author(AnswerAuthor::Temporary(TemporaryAnswerAuthor::new(
            "temporary user".to_string(),
            "temporary@example.com".to_string(),
        )))
    }

    #[tokio::test]
    async fn temporary_answer_with_existing_messages_rejects_message_without_external_side_effects()
    {
        let actor = user();
        let (form, answer) = form_and_temporary_answer();
        let form_id = *form.id();
        let answer_id = *answer.id();
        let existing_messages = vec![Message::new(
            *actor.id(),
            MessageBody::new("existing".to_string().try_into().unwrap()),
        )];
        let mut repositories = FormUseCaseTestRepositories::with_active_forms(vec![form]);
        repositories.answer_entry_repository = InMemoryAnswerEntryRepository::new(vec![answer]);
        let messages = InMemoryMessageThreadRepository::with_messages(answer_id, existing_messages);
        let messages_before_post = messages.stored_messages();
        let publisher = RecordingPublisher::default();
        let notificator = FailingNotificator::default();
        let usecase = MessageUseCase {
            notification_repository: &repositories.notification_repository,
            active_form_repository: &repositories.active_form_repository,
            user_repository: &repositories.user_repository,
            answer_entry_repository: &repositories.answer_entry_repository,
            message_thread_repository: &messages,
            application_event_publisher: Some(&publisher),
        };

        let result = usecase
            .post_message(
                &actor,
                form_id,
                MessageBody::new("must not be appended".to_string().try_into().unwrap()),
                answer_id,
                &notificator,
            )
            .await;

        assert_eq!(
            result,
            Err(errors::domain::DomainError::MessagePostingNotSupportedForTemporaryAnswer.into())
        );
        assert_eq!(messages.stored_messages(), messages_before_post);
        assert!(publisher.0.lock().unwrap().is_empty());
        assert!(!notificator.called.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn authenticated_answer_accepts_initial_and_follow_up_messages() {
        let actor = user();
        let (form, answer) = form_and_answer(&actor);
        let form_id = *form.id();
        let answer_id = *answer.id();
        let mut repositories = FormUseCaseTestRepositories::with_active_forms(vec![form]);
        repositories.answer_entry_repository = InMemoryAnswerEntryRepository::new(vec![answer]);
        let messages = InMemoryMessageThreadRepository::default();
        let usecase = MessageUseCase {
            notification_repository: &repositories.notification_repository,
            active_form_repository: &repositories.active_form_repository,
            user_repository: &repositories.user_repository,
            answer_entry_repository: &repositories.answer_entry_repository,
            message_thread_repository: &messages,
            application_event_publisher: None,
        };

        for body in ["initial", "follow up"] {
            usecase
                .post_message(
                    &actor,
                    form_id,
                    MessageBody::new(body.to_string().try_into().unwrap()),
                    answer_id,
                    &NoopNotificator,
                )
                .await
                .unwrap();
        }

        assert_eq!(messages.message_count_for(answer_id), 2);
    }

    #[tokio::test]
    async fn empty_thread_reads_empty_and_unknown_message_mutations_return_message_not_found() {
        let actor = user();
        let (form, answer) = form_and_answer(&actor);
        let form_id = *form.id();
        let answer_id = *answer.id();
        let mut repositories = FormUseCaseTestRepositories::with_active_forms(vec![form]);
        repositories.answer_entry_repository = InMemoryAnswerEntryRepository::new(vec![answer]);
        let messages = InMemoryMessageThreadRepository::default();
        let usecase = MessageUseCase {
            notification_repository: &repositories.notification_repository,
            active_form_repository: &repositories.active_form_repository,
            user_repository: &repositories.user_repository,
            answer_entry_repository: &repositories.answer_entry_repository,
            message_thread_repository: &messages,
            application_event_publisher: None,
        };
        let unknown_message_id = MessageId::new();

        let fetched = usecase
            .get_messages(&actor, form_id, answer_id)
            .await
            .unwrap();
        let history = usecase
            .get_history(
                &actor,
                form_id,
                answer_id,
                PageRequest::first(PageLimit::default_limit()),
            )
            .await
            .unwrap();
        let update = usecase
            .update_message_body(
                &actor,
                form_id,
                answer_id,
                &unknown_message_id,
                Some(MessageBody::new("updated".to_string().try_into().unwrap())),
            )
            .await;
        let delete = usecase
            .delete_message(&actor, form_id, answer_id, &unknown_message_id)
            .await;

        assert!(fetched.is_empty());
        assert!(history.items().is_empty());
        assert_eq!(update, Err(Error::from(MessageNotFound)));
        assert_eq!(delete, Err(Error::from(MessageNotFound)));
    }

    #[tokio::test]
    async fn message_cud_publishes_saved_body_and_skips_empty_or_equal_updates() {
        let user = user();
        let (form, answer) = form_and_answer(&user);
        let form_id = *form.id();
        let answer_id = *answer.id();
        let mut repositories = FormUseCaseTestRepositories::with_active_forms(vec![form]);
        repositories.answer_entry_repository = InMemoryAnswerEntryRepository::new(vec![answer]);
        let messages = InMemoryMessageThreadRepository::default();
        let publisher = RecordingPublisher::default();
        let usecase = MessageUseCase {
            notification_repository: &repositories.notification_repository,
            active_form_repository: &repositories.active_form_repository,
            user_repository: &repositories.user_repository,
            answer_entry_repository: &repositories.answer_entry_repository,
            message_thread_repository: &messages,
            application_event_publisher: Some(&publisher),
        };

        let original = MessageBody::new("original".to_string().try_into().unwrap());
        usecase
            .post_message(
                &user,
                form_id,
                original.clone(),
                answer_id,
                &NoopNotificator,
            )
            .await
            .unwrap();
        let message_id = messages.only_message_id();
        usecase
            .update_message_body(&user, form_id, answer_id, &message_id, None)
            .await
            .unwrap();
        usecase
            .update_message_body(&user, form_id, answer_id, &message_id, Some(original))
            .await
            .unwrap();
        usecase
            .update_message_body(
                &user,
                form_id,
                answer_id,
                &message_id,
                Some(MessageBody::new("updated".to_string().try_into().unwrap())),
            )
            .await
            .unwrap();
        usecase
            .delete_message(&user, form_id, answer_id, &message_id)
            .await
            .unwrap();

        let events = publisher.0.lock().unwrap();
        assert!(matches!(
            events.as_slice(),
            [
                ApplicationEvent::MessageCreated { body: created, .. },
                ApplicationEvent::MessageUpdated { body: updated, .. },
                ApplicationEvent::MessageDeleted { body: deleted, .. }
            ] if created == "original" && updated == "updated" && deleted == "updated"
        ));
    }

    #[tokio::test]
    async fn message_created_is_published_before_individual_notification_failure() {
        unsafe { std::env::set_var("FRONTEND_URL", "https://example.com") };
        let actor = user();
        let recipient = AccountUser::new(
            "recipient".to_string(),
            UserId::from(Uuid::new_v4()),
            Role::StandardUser,
        );
        let recipient_id = *recipient.id();
        let (form, answer) = form_and_answer(&recipient);
        let form_id = *form.id();
        let answer_id = *answer.id();
        let mut repositories = FormUseCaseTestRepositories::with_active_forms(vec![form]);
        repositories.answer_entry_repository = InMemoryAnswerEntryRepository::new(vec![answer]);
        repositories.user_repository.save_user(recipient);
        let messages = InMemoryMessageThreadRepository::default();
        let publisher = RecordingPublisher::default();
        let notificator = FailingNotificator::default();
        let usecase = MessageUseCase {
            notification_repository: &repositories.notification_repository,
            active_form_repository: &repositories.active_form_repository,
            user_repository: &repositories.user_repository,
            answer_entry_repository: &repositories.answer_entry_repository,
            message_thread_repository: &messages,
            application_event_publisher: Some(&publisher),
        };

        let result = usecase
            .post_message(
                &actor,
                form_id,
                MessageBody::new("saved".to_string().try_into().unwrap()),
                answer_id,
                &notificator,
            )
            .await;

        assert!(result.is_err());
        assert!(notificator.called.load(Ordering::Relaxed));
        assert_eq!(*notificator.recipient.lock().unwrap(), Some(recipient_id));
        assert!(matches!(
            publisher.0.lock().unwrap().as_slice(),
            [ApplicationEvent::MessageCreated { body, .. }] if body == "saved"
        ));
    }
}
