use domain::{
    form::{
        answer::models::{
            AnswerAuthor, AnswerEntry, AnswerId, AnswerTitle, FormAnswerContent,
            PostedAnswerContents,
        },
        answer_entry_set::models::AnswerEntrySet,
        models::FormId,
        service::DefaultAnswerTitleDomainService,
    },
    repository::form::{
        active_form_repository::ActiveFormRepository,
        answer_entry_set_repository::AnswerEntrySetRepository,
        answer_label_repository::AnswerLabelRepository, comment_repository::CommentRepository,
    },
    repository::user_repository::UserRepository,
    types::{authorization_guard::AuthorizationGuard, authorization_guard_with_context::Read},
    user::models::{ActiveUser, Actor, TemporaryUser, User},
};
use errors::{
    Error,
    domain::DomainError,
    usecase::UseCaseError::{AnswerNotFound, FormNotFound},
};
use futures::{StreamExt, stream, try_join};

use crate::{
    models::{AnswerDetails, CommentWithAuthor},
    user_reference_resolver::resolve_user_references,
};

pub struct AnswerUseCase<
    'a,
    FormRepo: ActiveFormRepository,
    CommentRepo: CommentRepository,
    AnswerLabelRepo: AnswerLabelRepository,
    UserRepo: UserRepository,
    AnswerEntrySetRepo: AnswerEntrySetRepository,
> {
    pub active_form_repository: &'a FormRepo,
    pub comment_repository: &'a CommentRepo,
    pub answer_label_repository: &'a AnswerLabelRepo,
    pub user_repository: &'a UserRepo,
    pub answer_entry_set_repository: &'a AnswerEntrySetRepo,
}

impl<
    R1: ActiveFormRepository,
    R2: CommentRepository,
    R3: AnswerLabelRepository,
    R4: UserRepository,
    R5: AnswerEntrySetRepository,
> AnswerUseCase<'_, R1, R2, R3, R4, R5>
{
    async fn read_answer_entry_set_guard(
        &self,
        form_id: FormId,
        actor: &Actor,
    ) -> Result<AuthorizationGuard<AnswerEntrySet, Read>, Error> {
        let form_guard = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?;

        let form = form_guard.try_read(actor)?;
        let answer_entry_set_id = *form.answer_entry_set_id();

        let set_guard = self
            .answer_entry_set_repository
            .get(answer_entry_set_id)
            .await?
            .ok_or(FormNotFound)?;

        set_guard.try_read(actor)?;
        Ok(set_guard)
    }

    async fn read_answer_entry_set(
        &self,
        form_id: FormId,
        actor: &Actor,
    ) -> Result<AnswerEntrySet, Error> {
        self.read_answer_entry_set_guard(form_id, actor)
            .await?
            .try_into_read(actor)
            .map_err(Into::into)
    }

    async fn build_answer_details(
        &self,
        actor: &ActiveUser,
        form_answer: AnswerEntry,
        labels: Vec<domain::form::answer::models::AnswerLabel>,
        comments: Vec<domain::form::comment::models::Comment>,
    ) -> Result<AnswerDetails, Error> {
        let user_ids = form_answer
            .author()
            .authenticated_user_id()
            .into_iter()
            .chain(comments.iter().map(|comment| *comment.commented_by()))
            .collect();

        let users = resolve_user_references(self.user_repository, actor, user_ids).await?;

        let author = match form_answer.author() {
            AnswerAuthor::AuthenticatedUser(user_id) => User::ActiveUser(
                users
                    .get(user_id)
                    .cloned()
                    .ok_or(Error::from(errors::usecase::UseCaseError::UserNotFound))?,
            ),
            AnswerAuthor::TemporaryUser(temporary_user) => {
                User::TemporaryUser(temporary_user.clone())
            }
        };

        let comments = comments
            .into_iter()
            .map(|comment| {
                let commented_by = users
                    .get(comment.commented_by())
                    .cloned()
                    .ok_or(Error::from(errors::usecase::UseCaseError::UserNotFound))?;
                Ok::<_, Error>(CommentWithAuthor {
                    comment,
                    commented_by,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(AnswerDetails {
            form_answer,
            author,
            labels,
            comments,
        })
    }

    pub async fn post_answers(
        &self,
        user: ActiveUser,
        form_id: FormId,
        answers: Vec<FormAnswerContent>,
    ) -> Result<(), Error> {
        let actor = Actor::from(user.clone());
        let answer_entry_set_guard = self.read_answer_entry_set_guard(form_id, &actor).await?;
        let answer_entry_set = answer_entry_set_guard.try_read(&actor)?;

        let form_guard = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?;
        let form = form_guard.try_read(&actor)?;
        let questions = form.questions().as_slice().to_vec();
        let posted_answers = PostedAnswerContents::try_new(&questions, answers)?;

        let title = DefaultAnswerTitleDomainService::<R1>::to_answer_title_from_questions(
            answer_entry_set.default_answer_title().to_owned(),
            &questions,
            &posted_answers,
            user.name(),
        )?;

        let author = AnswerAuthor::AuthenticatedUser(*user.id());
        if !answer_entry_set.can_accept_answer(&author, &actor) {
            return Err(Error::from(DomainError::Forbidden));
        }

        let answer_entry = AnswerEntry::new(author, form_id, title, posted_answers);

        self.answer_entry_set_repository
            .add_entry(&answer_entry_set_guard, &answer_entry, &actor)
            .await
    }

    pub async fn post_temporary_answers(
        &self,
        temporary_user: TemporaryUser,
        form_id: FormId,
        answers: Vec<FormAnswerContent>,
    ) -> Result<(), Error> {
        let actor = Actor::from(temporary_user.clone());
        let anonymous_actor = Actor::from(User::Anonymous);
        let answer_entry_set_guard = self
            .read_answer_entry_set_guard(form_id, &anonymous_actor)
            .await?;
        let answer_entry_set = answer_entry_set_guard.try_read(&anonymous_actor)?;

        let form_guard = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?;
        let form = form_guard.try_read(&Actor::from(User::Anonymous))?;
        let questions = form.questions().as_slice().to_vec();
        let posted_answers = PostedAnswerContents::try_new(&questions, answers)?;

        let title = DefaultAnswerTitleDomainService::<R1>::to_answer_title_from_questions(
            answer_entry_set.default_answer_title().to_owned(),
            &questions,
            &posted_answers,
            temporary_user.name(),
        )?;

        let author = AnswerAuthor::TemporaryUser(temporary_user);
        if !answer_entry_set.can_accept_answer(&author, &actor) {
            return Err(Error::from(DomainError::Forbidden));
        }

        let answer_entry = AnswerEntry::new(author, form_id, title, posted_answers);

        self.answer_entry_set_repository
            .add_entry(&answer_entry_set_guard, &answer_entry, &actor)
            .await
    }

    pub async fn get_answers(
        &self,
        form_id: FormId,
        answer_id: AnswerId,
        user: &ActiveUser,
    ) -> Result<AnswerDetails, Error> {
        let actor = Actor::from(user.clone());
        let answer_entry_set = self.read_answer_entry_set(form_id, &actor).await?;

        let form_answer = answer_entry_set
            .read_entry(answer_id, &actor)
            .map_err(|error| match error {
                DomainError::NotFound => Error::from(AnswerNotFound),
                error => Error::from(error),
            })?
            .clone();

        let (labels, comments) = try_join!(
            self.answer_label_repository
                .get_labels_for_answers_by_answer_id(answer_id),
            self.comment_repository.get_comments(answer_id)
        )?;

        let labels = labels
            .into_iter()
            .map(|label| label.try_into_read(&actor))
            .collect::<Result<Vec<_>, _>>()?;

        self.build_answer_details(user, form_answer, labels, comments)
            .await
    }

    pub async fn get_answers_by_form_id(
        &self,
        form_id: FormId,
        actor: &ActiveUser,
    ) -> Result<Vec<AnswerDetails>, Error> {
        let actor_ref = Actor::from(actor.clone());
        let answer_entry_set = self.read_answer_entry_set(form_id, &actor_ref).await?;

        let visible_answers: Vec<AnswerEntry> = answer_entry_set
            .readable_entries(&actor_ref)
            .into_iter()
            .cloned()
            .collect();

        stream::iter(visible_answers)
            .then(|form_answer| async {
                let answer_id = *form_answer.id();
                let (labels, comments) = try_join!(
                    self.answer_label_repository
                        .get_labels_for_answers_by_answer_id(answer_id),
                    self.comment_repository.get_comments(answer_id)
                )?;

                let labels = labels
                    .into_iter()
                    .map(|label| label.try_into_read(&actor_ref))
                    .collect::<Result<Vec<_>, _>>()?;

                self.build_answer_details(actor, form_answer, labels, comments)
                    .await
            })
            .collect::<Vec<Result<AnswerDetails, Error>>>()
            .await
            .into_iter()
            .collect()
    }

    pub async fn get_all_answers(&self, user: &ActiveUser) -> Result<Vec<AnswerDetails>, Error> {
        let actor_ref = Actor::from(user.clone());
        let visible_answers = self
            .answer_entry_set_repository
            .list_all()
            .await?
            .into_iter()
            .flat_map(|set_guard| {
                set_guard
                    .try_into_read(&actor_ref)
                    .map(|set| {
                        set.readable_entries(&actor_ref)
                            .into_iter()
                            .cloned()
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();

        stream::iter(visible_answers)
            .then(|form_answer| {
                let user = user.clone();
                async move {
                    let actor_ref = Actor::from(user.clone());

                    let answer_id = *form_answer.id();
                    let (labels, comments) = try_join!(
                        self.answer_label_repository
                            .get_labels_for_answers_by_answer_id(answer_id),
                        self.comment_repository.get_comments(answer_id)
                    )?;

                    let labels = labels
                        .into_iter()
                        .map(|label| label.try_into_read(&actor_ref))
                        .collect::<Result<Vec<_>, _>>()?;

                    self.build_answer_details(&user, form_answer, labels, comments)
                        .await
                }
            })
            .collect::<Vec<Result<AnswerDetails, Error>>>()
            .await
            .into_iter()
            .filter(Result::is_ok)
            .collect()
    }

    pub async fn update_answer_meta(
        &self,
        form_id: FormId,
        answer_id: AnswerId,
        actor: &ActiveUser,
        title: Option<AnswerTitle>,
    ) -> Result<AnswerDetails, Error> {
        let actor_ref = Actor::from(actor.clone());
        let answer_entry_set_guard = self
            .read_answer_entry_set_guard(form_id, &actor_ref)
            .await?;
        let answer_entry_set = answer_entry_set_guard.try_read(&actor_ref)?;

        let mut form_answer = answer_entry_set
            .read_entry(answer_id, &actor_ref)
            .map_err(|error| match error {
                DomainError::NotFound => Error::from(AnswerNotFound),
                error => Error::from(error),
            })?
            .clone();

        if let Some(title) = title {
            form_answer = answer_entry_set.change_entry_title(answer_id, &actor_ref, title)?;
            self.answer_entry_set_repository
                .update_entry(&answer_entry_set_guard, &form_answer, &actor_ref)
                .await?;
        }

        let (labels, comments) = try_join!(
            self.answer_label_repository
                .get_labels_for_answers_by_answer_id(answer_id),
            self.comment_repository.get_comments(answer_id)
        )?;

        let labels = labels
            .into_iter()
            .map(|label| label.try_into_read(&actor_ref))
            .collect::<Result<Vec<_>, _>>()?;

        self.build_answer_details(actor, form_answer, labels, comments)
            .await
    }
}
