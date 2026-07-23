use chrono::Utc;
use domain::form::comment::CommentContent;
use domain::form::models::FormId;
use domain::{
    account::models::AccountUser,
    auth::Actor,
    form::{
        answer::{AnswerEntry, AnswerId},
        comment::{Comment, CommentHistoryEntry, CommentHistoryPagePosition, CommentId},
    },
    pagination::{Page, PageRequest},
    repository::form::{
        active_form_repository::ActiveFormRepository,
        answer_entry_repository::AnswerEntryRepository, comment_repository::CommentRepository,
    },
    repository::user_repository::UserRepository,
    types::authorization_guard::{Allowed, Read},
};
use errors::{
    Error,
    usecase::UseCaseError::{AnswerNotFound, CommentNotFound, FormNotFound, UserNotFound},
};

use crate::{
    application_event::{ApplicationActor, ApplicationEvent, ApplicationEventPublisher},
    models::CommentWithAuthor,
    user_reference_resolver::resolve_user_references,
};

pub struct CommentUseCase<
    'a,
    FormRepo: ActiveFormRepository,
    UserRepo: UserRepository,
    AnswerEntryRepo: AnswerEntryRepository,
    CommentRepo: CommentRepository,
> {
    pub active_form_repository: &'a FormRepo,
    pub user_repository: &'a UserRepo,
    pub answer_entry_repository: &'a AnswerEntryRepo,
    pub comment_repository: &'a CommentRepo,
    pub application_event_publisher: Option<&'a dyn ApplicationEventPublisher>,
}

impl<R1: ActiveFormRepository, R2: UserRepository, R3: AnswerEntryRepository, R4: CommentRepository>
    CommentUseCase<'_, R1, R2, R3, R4>
{
    /// フォームと回答の読み取り認可を通過した [`AnswerEntry`] のガードを取得する。
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

    async fn build_comments_with_authors(
        &self,
        actor: &AccountUser,
        comments: Vec<Comment>,
    ) -> Result<Vec<CommentWithAuthor>, Error> {
        let user_ids = comments.iter().map(|c| *c.commented_by()).collect();
        let users = resolve_user_references(self.user_repository, actor, user_ids).await?;

        comments
            .into_iter()
            .map(|comment| {
                let commented_by = users
                    .get(comment.commented_by())
                    .cloned()
                    .ok_or(Error::from(UserNotFound))?;
                Ok(CommentWithAuthor {
                    comment,
                    commented_by,
                })
            })
            .collect()
    }

    pub async fn get_comments(
        &self,
        actor: &AccountUser,
        form_id: FormId,
        answer_id: AnswerId,
    ) -> Result<Vec<CommentWithAuthor>, Error> {
        let actor_user = Actor::from(actor.clone());
        let entry = self
            .read_answer_entry(&actor_user, form_id, answer_id)
            .await?;

        let comments = self
            .comment_repository
            .find_by_answer(&entry)
            .await?
            .into_iter()
            .map(|comment| comment.into_inner())
            .collect::<Vec<_>>();

        self.build_comments_with_authors(actor, comments).await
    }

    pub async fn post_comment(
        &self,
        actor: &AccountUser,
        form_id: FormId,
        answer_id: AnswerId,
        content: CommentContent,
    ) -> Result<(), Error> {
        let actor_user = Actor::from(actor.clone());
        let entry = self
            .read_answer_entry(&actor_user, form_id, answer_id)
            .await?;

        let comment = entry.create_comment(content)?;
        let comment_id = comment.comment_id().to_string();
        let content = comment.content().to_owned().into_inner().into_inner();

        self.comment_repository.create(comment).await?;
        if let Some(publisher) = self.application_event_publisher {
            publisher.publish(ApplicationEvent::CommentCreated {
                actor: ApplicationActor::from(actor),
                form_id: form_id.to_string(),
                answer_id: answer_id.to_string(),
                comment_id,
                content,
            });
        }

        Ok(())
    }

    pub async fn get_history(
        &self,
        actor: &AccountUser,
        form_id: FormId,
        answer_id: AnswerId,
        request: PageRequest<CommentHistoryPagePosition>,
    ) -> Result<Page<Allowed<CommentHistoryEntry, Read>, CommentHistoryPagePosition>, Error> {
        let entry = self
            .read_answer_entry(&Actor::from(actor.clone()), form_id, answer_id)
            .await?;
        self.comment_repository.history(&entry, request).await
    }

    pub async fn update_comment(
        &self,
        actor: &AccountUser,
        form_id: FormId,
        answer_id: AnswerId,
        comment_id: CommentId,
        content: Option<CommentContent>,
    ) -> Result<(), Error> {
        let actor_user = Actor::from(actor.clone());
        let entry = self
            .read_answer_entry(&actor_user, form_id, answer_id)
            .await?;
        let current_comment = self
            .comment_repository
            .find_by_answer(&entry)
            .await?
            .into_iter()
            .find(|comment| *comment.value().comment_id() == comment_id)
            .ok_or(Error::from(CommentNotFound))?;

        if let Some(content) = content {
            if current_comment.content() == &content {
                return Ok(());
            }
            let content_for_event = content.to_owned().into_inner().into_inner();
            let comment_id = current_comment.comment_id().to_string();
            let updated = entry.update_comment(current_comment.into_inner(), content)?;
            self.comment_repository.update(updated, Utc::now()).await?;
            if let Some(publisher) = self.application_event_publisher {
                publisher.publish(ApplicationEvent::CommentUpdated {
                    actor: ApplicationActor::from(actor),
                    form_id: form_id.to_string(),
                    answer_id: answer_id.to_string(),
                    comment_id,
                    content: content_for_event,
                });
            }
        }

        Ok(())
    }

    pub async fn delete_comment(
        &self,
        actor: &AccountUser,
        form_id: FormId,
        answer_id: AnswerId,
        comment_id: CommentId,
    ) -> Result<(), Error> {
        let actor_user = Actor::from(actor.clone());
        let entry = self
            .read_answer_entry(&actor_user, form_id, answer_id)
            .await?;
        let comment = self
            .comment_repository
            .find_by_answer(&entry)
            .await?
            .into_iter()
            .find(|comment| *comment.value().comment_id() == comment_id)
            .ok_or(Error::from(CommentNotFound))?;

        let comment_id = comment.comment_id().to_string();
        let content = comment.content().to_owned().into_inner().into_inner();

        let comment = entry.delete_comment(comment.into_inner(), Utc::now())?;

        self.comment_repository.delete(comment).await?;
        if let Some(publisher) = self.application_event_publisher {
            publisher.publish(ApplicationEvent::CommentDeleted {
                actor: ApplicationActor::from(actor),
                form_id: form_id.to_string(),
                answer_id: answer_id.to_string(),
                comment_id,
                content,
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use async_trait::async_trait;
    use domain::{
        account::models::{AccountUser, Role, UserId},
        form::{
            answer::{AnswerAuthor, AnswerEntry, AnswerId, AnswerTitle},
            comment::{CommentHistoryEntry, DeletedComment},
            models::{ActiveForm, FormDescription, FormTitle, QuestionSet},
            question::Question,
        },
        pagination::Page,
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
    struct InMemoryCommentRepository(Mutex<Vec<Comment>>);

    impl InMemoryCommentRepository {
        fn only_comment(&self) -> Comment {
            self.0.lock().unwrap().first().unwrap().clone()
        }
    }

    #[async_trait]
    impl CommentRepository for InMemoryCommentRepository {
        async fn create(&self, comment: Allowed<Comment, Create>) -> Result<(), Error> {
            self.0.lock().unwrap().push(comment.into_inner());
            Ok(())
        }

        async fn find_by_answer(
            &self,
            answer: &Allowed<AnswerEntry, Read>,
        ) -> Result<Vec<Allowed<Comment, Read>>, Error> {
            self.0
                .lock()
                .unwrap()
                .iter()
                .cloned()
                .map(|comment| answer.authorize_comment(comment).map_err(Into::into))
                .collect()
        }

        async fn update(
            &self,
            comment: Allowed<Comment, Update>,
            _updated_at: chrono::DateTime<Utc>,
        ) -> Result<(), Error> {
            let comment = comment.into_inner();
            let comment_id = *comment.comment_id();
            let mut comments = self.0.lock().unwrap();
            *comments
                .iter_mut()
                .find(|stored| *stored.comment_id() == comment_id)
                .unwrap() = comment;
            Ok(())
        }

        async fn delete(&self, comment: Allowed<DeletedComment, Create>) -> Result<(), Error> {
            let comment_id = *comment.comment().comment_id();
            self.0
                .lock()
                .unwrap()
                .retain(|stored| *stored.comment_id() != comment_id);
            Ok(())
        }

        async fn history(
            &self,
            _answer: &Allowed<AnswerEntry, Read>,
            _request: PageRequest<CommentHistoryPagePosition>,
        ) -> Result<Page<Allowed<CommentHistoryEntry, Read>, CommentHistoryPagePosition>, Error>
        {
            Ok(Page::new(Vec::new(), None))
        }

        async fn size(&self) -> Result<u32, Error> {
            Ok(self.0.lock().unwrap().len() as u32)
        }
    }

    fn user() -> AccountUser {
        AccountUser::new(
            "admin".to_string(),
            UserId::from(Uuid::new_v4()),
            Role::Administrator,
        )
    }

    fn form_and_answer(user: &AccountUser) -> (ActiveForm, AnswerEntry) {
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
                AnswerAuthor::AuthenticatedUser(*user.id()),
                Utc::now(),
                AnswerTitle::new(None),
                Vec::new(),
            )
        };
        (form, answer)
    }

    #[tokio::test]
    async fn comment_cud_publishes_saved_content_and_skips_empty_or_equal_updates() {
        let user = user();
        let (form, answer) = form_and_answer(&user);
        let form_id = *form.id();
        let answer_id = *answer.id();
        let mut repositories = FormUseCaseTestRepositories::with_active_forms(vec![form]);
        repositories.answer_entry_repository = InMemoryAnswerEntryRepository::new(vec![answer]);
        let comments = InMemoryCommentRepository::default();
        let publisher = RecordingPublisher::default();
        let usecase = CommentUseCase {
            active_form_repository: &repositories.active_form_repository,
            user_repository: &repositories.user_repository,
            answer_entry_repository: &repositories.answer_entry_repository,
            comment_repository: &comments,
            application_event_publisher: Some(&publisher),
        };

        let original = CommentContent::new("original".to_string().try_into().unwrap());
        usecase
            .post_comment(&user, form_id, answer_id, original.clone())
            .await
            .unwrap();
        let comment_id = *comments.only_comment().comment_id();
        usecase
            .update_comment(&user, form_id, answer_id, comment_id, None)
            .await
            .unwrap();
        usecase
            .update_comment(&user, form_id, answer_id, comment_id, Some(original))
            .await
            .unwrap();
        usecase
            .update_comment(
                &user,
                form_id,
                answer_id,
                comment_id,
                Some(CommentContent::new(
                    "updated".to_string().try_into().unwrap(),
                )),
            )
            .await
            .unwrap();
        usecase
            .delete_comment(&user, form_id, answer_id, comment_id)
            .await
            .unwrap();

        let events = publisher.0.lock().unwrap();
        assert!(matches!(
            events.as_slice(),
            [
                ApplicationEvent::CommentCreated { content: created, .. },
                ApplicationEvent::CommentUpdated { content: updated, .. },
                ApplicationEvent::CommentDeleted { content: deleted, .. }
            ] if created == "original" && updated == "updated" && deleted == "updated"
        ));
    }
}
