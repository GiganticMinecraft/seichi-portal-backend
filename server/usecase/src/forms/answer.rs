use domain::{
    form::{
        answer::models::{
            AnswerAuthor, AnswerEntry, AnswerId, AnswerLabel, AnswerTitle, FormAnswerContent,
            PostedAnswerContents,
        },
        answer_entry_set::models::AnswerEntrySet,
        models::FormId,
        service::DefaultAnswerTitleDomainService,
    },
    repository::form::{
        active_form_repository::ActiveFormRepository,
        answer_entry_set_repository::AnswerEntrySetRepository,
        answer_label_repository::AnswerLabelRepository,
    },
    repository::user_repository::UserRepository,
    types::authorization_guard::{Allowed, Read},
    user::models::{ActiveUser, Actor, TemporaryUser, User},
};
use errors::{
    Error,
    domain::DomainError,
    usecase::UseCaseError::{AnswerNotFound, FormNotFound},
};
use futures::{StreamExt, stream};

use crate::{models::AnswerDetails, user_reference_resolver::resolve_user_references};

pub struct AnswerUseCase<
    'a,
    FormRepo: ActiveFormRepository,
    AnswerLabelRepo: AnswerLabelRepository,
    UserRepo: UserRepository,
    AnswerEntrySetRepo: AnswerEntrySetRepository,
> {
    pub active_form_repository: &'a FormRepo,
    pub answer_label_repository: &'a AnswerLabelRepo,
    pub user_repository: &'a UserRepo,
    pub answer_entry_set_repository: &'a AnswerEntrySetRepo,
}

impl<
    R1: ActiveFormRepository,
    R2: AnswerLabelRepository,
    R3: UserRepository,
    R4: AnswerEntrySetRepository,
> AnswerUseCase<'_, R1, R2, R3, R4>
{
    async fn read_answer_entry_set_guard(
        &self,
        form_id: FormId,
        actor: &Actor,
    ) -> Result<Allowed<AnswerEntrySet, Read>, Error> {
        let form_guard = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?;

        form_guard.try_read(actor.clone())?;

        let set_guard = self
            .answer_entry_set_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?;

        set_guard.try_read(actor.clone()).map_err(Into::into)
    }

    async fn read_answer_entry_set(
        &self,
        form_id: FormId,
        actor: &Actor,
    ) -> Result<Allowed<AnswerEntrySet, Read>, Error> {
        self.read_answer_entry_set_guard(form_id, actor).await
    }

    async fn build_answer_details(
        &self,
        actor: &ActiveUser,
        form_id: FormId,
        form_answer: Allowed<AnswerEntry, Read>,
        labels: Vec<AnswerLabel>,
    ) -> Result<AnswerDetails, Error> {
        let user_ids = form_answer
            .author()
            .authenticated_user_id()
            .into_iter()
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

        Ok(AnswerDetails {
            form_id,
            form_answer: form_answer.into_inner(),
            author,
            labels,
        })
    }

    pub async fn post_answers(
        &self,
        user: ActiveUser,
        form_id: FormId,
        answers: Vec<FormAnswerContent>,
    ) -> Result<(), Error> {
        let actor = Actor::from(user.clone());
        let answer_entry_set = self.read_answer_entry_set_guard(form_id, &actor).await?;

        let form_guard = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?;
        let form = form_guard.try_read(actor.clone())?;
        let questions = form.value().questions().as_slice().to_vec();
        let posted_answers = PostedAnswerContents::try_new(&questions, answers)?;

        let title = DefaultAnswerTitleDomainService::to_answer_title_from_questions(
            answer_entry_set.value().default_answer_title().to_owned(),
            &questions,
            &posted_answers,
            user.name(),
        )?;

        let author = AnswerAuthor::AuthenticatedUser(*user.id());
        let answer_entry =
            answer_entry_set
                .value()
                .try_accept_answer(author, &actor, title, posted_answers)?;

        self.answer_entry_set_repository
            .add_entry(&answer_entry_set, &answer_entry)
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
        let answer_entry_set = self
            .read_answer_entry_set_guard(form_id, &anonymous_actor)
            .await?;

        let form_guard = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?;
        let form = form_guard.try_read(Actor::from(User::Anonymous))?;
        let questions = form.value().questions().as_slice().to_vec();
        let posted_answers = PostedAnswerContents::try_new(&questions, answers)?;

        let title = DefaultAnswerTitleDomainService::to_answer_title_from_questions(
            answer_entry_set.value().default_answer_title().to_owned(),
            &questions,
            &posted_answers,
            temporary_user.name(),
        )?;

        let author = AnswerAuthor::TemporaryUser(temporary_user);
        let answer_entry =
            answer_entry_set
                .value()
                .try_accept_answer(author, &actor, title, posted_answers)?;

        self.answer_entry_set_repository
            .add_entry(&answer_entry_set, &answer_entry)
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
            .read_entry(answer_id)
            .map_err(|error| match error {
                DomainError::NotFound => Error::from(AnswerNotFound),
                error => Error::from(error),
            })?;

        let labels = self
            .answer_label_repository
            .get_labels_for_answers_by_answer_id(answer_id)
            .await?
            .into_iter()
            .map(|label| {
                label
                    .try_read(actor.clone())
                    .map(|label| label.into_inner())
            })
            .collect::<Result<Vec<_>, _>>()?;

        self.build_answer_details(user, form_id, form_answer, labels)
            .await
    }

    pub async fn get_answers_by_form_id(
        &self,
        form_id: FormId,
        actor: &ActiveUser,
    ) -> Result<Vec<AnswerDetails>, Error> {
        let actor_ref = Actor::from(actor.clone());
        let answer_entry_set = self.read_answer_entry_set(form_id, &actor_ref).await?;

        let visible_answers = answer_entry_set.readable_entries();

        stream::iter(visible_answers)
            .then(|form_answer| async {
                let answer_id = *form_answer.id();
                let labels = self
                    .answer_label_repository
                    .get_labels_for_answers_by_answer_id(answer_id)
                    .await?
                    .into_iter()
                    .map(|label| {
                        label
                            .try_read(actor_ref.clone())
                            .map(|label| label.into_inner())
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                self.build_answer_details(actor, form_id, form_answer, labels)
                    .await
            })
            .collect::<Vec<Result<AnswerDetails, Error>>>()
            .await
            .into_iter()
            .collect()
    }

    pub async fn get_all_answers(&self, user: &ActiveUser) -> Result<Vec<AnswerDetails>, Error> {
        let actor_ref = Actor::from(user.clone());
        let visible_answers: Vec<(FormId, Allowed<AnswerEntry, Read>)> = self
            .answer_entry_set_repository
            .list_all()
            .await?
            .into_iter()
            .flat_map(|set_guard| {
                set_guard
                    .try_read(actor_ref.clone())
                    .map(|set| {
                        let form_id = *set.value().form_id();
                        set.readable_entries()
                            .into_iter()
                            .map(move |entry| (form_id, entry))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>();

        stream::iter(visible_answers)
            .then(|(form_id, form_answer)| {
                let user = user.clone();
                async move {
                    let actor_ref = Actor::from(user.clone());

                    let answer_id = *form_answer.id();
                    let labels = self
                        .answer_label_repository
                        .get_labels_for_answers_by_answer_id(answer_id)
                        .await?
                        .into_iter()
                        .map(|label| {
                            label
                                .try_read(actor_ref.clone())
                                .map(|label| label.into_inner())
                        })
                        .collect::<Result<Vec<_>, _>>()?;

                    self.build_answer_details(&user, form_id, form_answer, labels)
                        .await
                }
            })
            .collect::<Vec<Result<AnswerDetails, Error>>>()
            .await
            .into_iter()
            .filter(|result| {
                !matches!(
                    result,
                    Err(Error::Domain {
                        source: DomainError::Forbidden
                    })
                )
            })
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
        let answer_entry_set = self
            .read_answer_entry_set_guard(form_id, &actor_ref)
            .await?;

        let form_answer = match title {
            Some(title) => {
                let answer_entry_set = self
                    .answer_entry_set_repository
                    .get(form_id)
                    .await?
                    .ok_or(FormNotFound)?
                    .into_update()
                    .try_update(actor_ref.clone())?;
                let form_answer = answer_entry_set.change_entry_title(answer_id, title)?;

                self.answer_entry_set_repository
                    .update_entry(&answer_entry_set, &form_answer)
                    .await?;

                self.read_answer_entry_set(form_id, &actor_ref)
                    .await?
                    .read_entry(answer_id)
                    .map_err(|error| match error {
                        DomainError::NotFound => Error::from(AnswerNotFound),
                        error => Error::from(error),
                    })?
            }
            None => answer_entry_set
                .read_entry(answer_id)
                .map_err(|error| match error {
                    DomainError::NotFound => Error::from(AnswerNotFound),
                    error => Error::from(error),
                })?,
        };

        let labels = self
            .answer_label_repository
            .get_labels_for_answers_by_answer_id(answer_id)
            .await?
            .into_iter()
            .map(|label| {
                label
                    .try_read(actor_ref.clone())
                    .map(|label| label.into_inner())
            })
            .collect::<Result<Vec<_>, _>>()?;

        self.build_answer_details(actor, form_id, form_answer, labels)
            .await
    }
}
