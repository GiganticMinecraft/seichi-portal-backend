use chrono::Utc;
use domain::{
    account::models::AccountUser,
    auth::Actor,
    form::{
        answer::{AnswerEntry, AnswerId},
        comment::{
            Comment, CommentContent, CommentHistoryEntry, CommentHistoryPagePosition, CommentId,
        },
        comment_thread::CommentThread,
        models::{ActiveForm, FormId},
    },
    pagination::{Page, PageRequest},
    repository::{
        form::{
            active_form_repository::ActiveFormRepository,
            answer_entry_repository::AnswerEntryRepository,
            comment_thread_repository::CommentThreadRepository,
        },
        form_submission_restriction_repository::FormSubmissionRestrictionRepository,
        user_repository::UserRepository,
    },
    types::authorization_guard::{Allowed, AuthorizationGuard, Read, Update},
};
use errors::{
    Error,
    domain::DomainError,
    usecase::UseCaseError::{AnswerNotFound, FormNotFound, UserNotFound},
};

use crate::{
    application_event::{ApplicationActor, ApplicationEvent, ApplicationEventPublisher},
    models::{CommentAuthor, CommentWithAuthor},
    user_reference_resolver::resolve_user_references,
};

pub struct CommentUseCase<
    'a,
    FormRepo: ActiveFormRepository,
    UserRepo: UserRepository,
    AnswerEntryRepo: AnswerEntryRepository,
    CommentThreadRepo: CommentThreadRepository,
> {
    pub active_form_repository: &'a FormRepo,
    pub user_repository: &'a UserRepo,
    pub answer_entry_repository: &'a AnswerEntryRepo,
    pub comment_thread_repository: &'a CommentThreadRepo,
    pub application_event_publisher: Option<&'a dyn ApplicationEventPublisher>,
}

impl<
    R1: ActiveFormRepository,
    R2: UserRepository,
    R3: AnswerEntryRepository,
    R4: CommentThreadRepository,
> CommentUseCase<'_, R1, R2, R3, R4>
{
    async fn readable_form_and_answer(
        &self,
        actor: &Actor,
        form_id: FormId,
        answer_id: AnswerId,
    ) -> Result<(Allowed<ActiveForm, Read>, AnswerEntry), Error> {
        let form = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?
            .try_read(actor.clone())?;
        let answer = self
            .answer_entry_repository
            .get(&form, answer_id)
            .await?
            .ok_or(AnswerNotFound)?
            .into_inner();
        Ok((form, answer))
    }

    async fn comment_thread_for_answer(
        &self,
        actor: &Actor,
        form_id: FormId,
        answer_id: AnswerId,
    ) -> Result<Allowed<CommentThread, Read>, Error> {
        let (form, answer) = self
            .readable_form_and_answer(actor, form_id, answer_id)
            .await?;
        self.comment_thread_repository
            .get_for_answer(&form, answer)
            .await
    }

    async fn comment_thread_with_comments_for_answer(
        &self,
        actor: &Actor,
        form_id: FormId,
        answer_id: AnswerId,
    ) -> Result<Allowed<CommentThread, Read>, Error> {
        let (form, answer) = self
            .readable_form_and_answer(actor, form_id, answer_id)
            .await?;
        self.comment_thread_repository
            .get_with_comments_for_answer(&form, answer)
            .await
    }

    async fn build_comments_with_authors(
        &self,
        actor: &AccountUser,
        comments: Vec<Comment>,
    ) -> Result<Vec<CommentWithAuthor>, Error> {
        let user_ids = comments
            .iter()
            .filter_map(|comment| comment.commented_by().copied())
            .collect();
        let users = resolve_user_references(self.user_repository, actor, user_ids).await?;
        comments
            .into_iter()
            .map(|comment| {
                let commented_by = match comment.commented_by() {
                    Some(user_id) => CommentAuthor::Portal(
                        users
                            .get(user_id)
                            .cloned()
                            .ok_or(Error::from(UserNotFound))?,
                    ),
                    None => CommentAuthor::ImportedFromRedmine(
                        comment
                            .redmine_author()
                            .cloned()
                            .ok_or(Error::from(UserNotFound))?,
                    ),
                };
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
        let thread = self
            .comment_thread_with_comments_for_answer(
                &Actor::from(actor.clone()),
                form_id,
                answer_id,
            )
            .await?;
        self.build_comments_with_authors(actor, thread.comments().to_vec())
            .await
    }

    pub async fn post_comment(
        &self,
        actor: &AccountUser,
        form_id: FormId,
        answer_id: AnswerId,
        content: CommentContent,
        restriction_repository: &impl FormSubmissionRestrictionRepository,
    ) -> Result<(), Error> {
        super::submission::authorize_form_submission(actor.clone(), restriction_repository).await?;
        let (form, answer) = self
            .readable_form_and_answer(&Actor::from(actor.clone()), form_id, answer_id)
            .await?;
        let thread = self
            .comment_thread_repository
            .get_for_answer(&form, answer)
            .await?;
        let comment = AuthorizationGuard::<_, Update>::from(thread.into_inner())
            .try_update(Actor::from(actor.clone()))?
            .create_comment(content)?;
        let comment_id = comment.comment_id().to_string();
        let content = comment.content().to_owned().into_inner().into_inner();
        self.comment_thread_repository
            .create(&form, comment)
            .await?;
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
        let thread = self
            .comment_thread_for_answer(&Actor::from(actor.clone()), form_id, answer_id)
            .await?;
        self.comment_thread_repository
            .history(&thread, request)
            .await
    }

    pub async fn update_comment(
        &self,
        actor: &AccountUser,
        form_id: FormId,
        answer_id: AnswerId,
        comment_id: CommentId,
        content: Option<CommentContent>,
    ) -> Result<(), Error> {
        if let Some(content) = content {
            let (form, answer) = self
                .readable_form_and_answer(&Actor::from(actor.clone()), form_id, answer_id)
                .await?;
            let thread = self
                .comment_thread_repository
                .get_with_comments_for_answer(&form, answer)
                .await?;
            let authorized_thread = AuthorizationGuard::<_, Update>::from(thread.into_inner())
                .try_update(Actor::from(actor.clone()))?;
            let current_content = authorized_thread
                .find_comment(comment_id)
                .ok_or(DomainError::NotFound)?
                .content()
                .clone();
            let updated = authorized_thread.authorize_comment_update(comment_id, content)?;
            if current_content == *updated.content() {
                return Ok(());
            }
            let content = updated.content().to_owned().into_inner().into_inner();
            let comment_id = updated.comment_id().to_string();
            self.comment_thread_repository
                .update(&form, updated, Utc::now())
                .await?;
            if let Some(publisher) = self.application_event_publisher {
                publisher.publish(ApplicationEvent::CommentUpdated {
                    actor: ApplicationActor::from(actor),
                    form_id: form_id.to_string(),
                    answer_id: answer_id.to_string(),
                    comment_id,
                    content,
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
        let (form, answer) = self
            .readable_form_and_answer(&Actor::from(actor.clone()), form_id, answer_id)
            .await?;
        let thread = self
            .comment_thread_repository
            .get_with_comments_for_answer(&form, answer)
            .await?;
        let comment = AuthorizationGuard::<_, Update>::from(thread.into_inner())
            .try_update(Actor::from(actor.clone()))?
            .authorize_comment_delete(comment_id, Utc::now())?;
        let content = comment
            .comment()
            .content()
            .to_owned()
            .into_inner()
            .into_inner();
        self.comment_thread_repository
            .delete(&form, comment)
            .await?;
        if let Some(publisher) = self.application_event_publisher {
            publisher.publish(ApplicationEvent::CommentDeleted {
                actor: ApplicationActor::from(actor),
                form_id: form_id.to_string(),
                answer_id: answer_id.to_string(),
                comment_id: comment_id.to_string(),
                content,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use domain::{
        account::models::{Role, UserId},
        form::{
            answer::{
                AnswerAuthor, AnswerPublication, AnswerSettings, AnswerTitle, AnswerVisibility,
            },
            comment::DeletedComment,
            models::{FormDescription, FormTitle, QuestionSet},
            question::Question,
        },
        repository::form::comment_thread_repository::CommentThreadRepository,
        types::authorization_guard::{Create, Update},
    };
    use types::non_empty_vec::NonEmptyVec;
    use uuid::Uuid;

    use crate::test_utils::repositories::{
        FormUseCaseTestRepositories, InMemoryAnswerEntryRepository,
    };

    struct ThreadRepository;

    #[async_trait]
    impl CommentThreadRepository for ThreadRepository {
        async fn get_for_answer(
            &self,
            form: &Allowed<ActiveForm, Read>,
            answer: AnswerEntry,
        ) -> Result<Allowed<CommentThread, Read>, Error> {
            form.comment_thread(answer).map_err(Into::into)
        }

        async fn get_with_comments_for_answer(
            &self,
            form: &Allowed<ActiveForm, Read>,
            answer: AnswerEntry,
        ) -> Result<Allowed<CommentThread, Read>, Error> {
            form.comment_thread_with_comments(answer, Vec::new())
                .map_err(Into::into)
        }

        async fn create(
            &self,
            _form: &Allowed<ActiveForm, Read>,
            _comment: Allowed<Comment, Create>,
        ) -> Result<(), Error> {
            Err(DomainError::Forbidden.into())
        }

        async fn update(
            &self,
            _form: &Allowed<ActiveForm, Read>,
            _comment: Allowed<Comment, Update>,
            _updated_at: chrono::DateTime<Utc>,
        ) -> Result<(), Error> {
            Err(DomainError::Forbidden.into())
        }

        async fn delete(
            &self,
            _form: &Allowed<ActiveForm, Read>,
            _comment: Allowed<DeletedComment, Create>,
        ) -> Result<(), Error> {
            Err(DomainError::Forbidden.into())
        }

        async fn history(
            &self,
            _comment_thread: &Allowed<CommentThread, Read>,
            _request: PageRequest<CommentHistoryPagePosition>,
        ) -> Result<Page<Allowed<CommentHistoryEntry, Read>, CommentHistoryPagePosition>, Error>
        {
            unreachable!("private thread cannot read comment history")
        }

        async fn size(&self) -> Result<u32, Error> {
            Ok(0)
        }
    }

    fn private_form_and_answer(author: &AccountUser) -> (ActiveForm, AnswerEntry) {
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
        )
        .change_answer_settings(
            AnswerSettings::default().change_visibility(AnswerVisibility::PRIVATE),
        );
        let answer = unsafe {
            AnswerEntry::from_raw_parts(
                AnswerId::new(),
                *form.id(),
                AnswerAuthor::AuthenticatedUser(*author.id()),
                Utc::now(),
                AnswerTitle::new(None),
                AnswerPublication::PUBLIC,
                Vec::new(),
            )
        };
        (form, answer)
    }

    #[tokio::test]
    async fn private_answer_author_cannot_list_history_or_mutate_comments() {
        let author = AccountUser::new(
            "author".to_string(),
            UserId::from(Uuid::new_v4()),
            Role::StandardUser,
        );
        let (form, answer) = private_form_and_answer(&author);
        let form_id = *form.id();
        let answer_id = *answer.id();
        let mut repositories = FormUseCaseTestRepositories::with_active_forms(vec![form]);
        repositories.answer_entry_repository = InMemoryAnswerEntryRepository::new(vec![answer]);
        let thread_repository = ThreadRepository;
        let use_case = CommentUseCase {
            active_form_repository: &repositories.active_form_repository,
            user_repository: &repositories.user_repository,
            answer_entry_repository: &repositories.answer_entry_repository,
            comment_thread_repository: &thread_repository,
            application_event_publisher: None,
        };

        let forbidden = Err::<(), Error>(DomainError::Forbidden.into());
        assert_eq!(
            use_case
                .get_comments(&author, form_id, answer_id)
                .await
                .map(|_| ()),
            forbidden
        );
        assert_eq!(
            use_case
                .get_history(
                    &author,
                    form_id,
                    answer_id,
                    PageRequest::first(domain::pagination::PageLimit::default_limit()),
                )
                .await
                .map(|_| ()),
            forbidden
        );
        assert_eq!(
            use_case
                .post_comment(
                    &author,
                    form_id,
                    answer_id,
                    CommentContent::new("comment".to_string().try_into().unwrap()),
                    &repositories.form_submission_restriction_repository,
                )
                .await,
            forbidden
        );
        assert_eq!(
            use_case
                .update_comment(
                    &author,
                    form_id,
                    answer_id,
                    CommentId::new(),
                    Some(CommentContent::new(
                        "updated".to_string().try_into().unwrap()
                    )),
                )
                .await,
            forbidden
        );
        assert_eq!(
            use_case
                .delete_comment(&author, form_id, answer_id, CommentId::new())
                .await,
            forbidden
        );
    }
}
