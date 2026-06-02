use domain::{
    form::{
        answer::models::{
            AnswerAuthor, AnswerEntry, AnswerId, AnswerLabel, AnswerTitle, FormAnswerContent,
            PostedAnswerContents,
        },
        answer_entry_set::models::AnswerEntrySet,
        models::{ActiveForm, FormId},
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
use futures::{StreamExt, TryStreamExt, stream};
use std::collections::HashMap;

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
    /// フォームのガードと、それに紐づく回答集合のガードを取得する。
    ///
    /// フォームの読み取り認可を通過したうえで [`AnswerEntrySet`] の所属を検証し、
    /// 個々の回答の認可は返り値の [`Allowed<ActiveForm, Read>`] を起点に行う。
    async fn read_form_and_entry_set(
        &self,
        form_id: FormId,
        actor: &Actor,
    ) -> Result<(Allowed<ActiveForm, Read>, Allowed<AnswerEntrySet, Read>), Error> {
        let form = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?
            .try_read(actor.clone())?;

        let answer_entry_set = self
            .answer_entry_set_repository
            .get_read(&form)
            .await?
            .ok_or(FormNotFound)?;

        Ok((form, answer_entry_set))
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

        let form_guard = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?;
        let form = form_guard.try_read(actor.clone())?;
        let questions = form.value().questions().as_slice().to_vec();
        let posted_answers = PostedAnswerContents::try_new(&questions, answers)?;

        let title = DefaultAnswerTitleDomainService::to_answer_title_from_questions(
            form.value()
                .answer_settings()
                .default_answer_title()
                .to_owned(),
            &questions,
            &posted_answers,
            user.name(),
        )?;

        let author = AnswerAuthor::AuthenticatedUser(*user.id());
        let answer_entry = form.try_accept_answer(author, title, posted_answers)?;

        let answer_entry_set = self
            .answer_entry_set_repository
            .get_read(&form)
            .await?
            .ok_or(FormNotFound)?;

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

        let form_guard = self
            .active_form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?;
        let form = form_guard.try_read(actor.clone())?;
        let questions = form.value().questions().as_slice().to_vec();
        let posted_answers = PostedAnswerContents::try_new(&questions, answers)?;

        let title = DefaultAnswerTitleDomainService::to_answer_title_from_questions(
            form.value()
                .answer_settings()
                .default_answer_title()
                .to_owned(),
            &questions,
            &posted_answers,
            temporary_user.name(),
        )?;

        let author = AnswerAuthor::TemporaryUser(temporary_user);
        let answer_entry = form.try_accept_answer(author, title, posted_answers)?;

        let answer_entry_set = self
            .answer_entry_set_repository
            .get_read(&form)
            .await?
            .ok_or(FormNotFound)?;

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
        let (form, answer_entry_set) = self.read_form_and_entry_set(form_id, &actor).await?;

        let form_answer =
            form.read_entry(&answer_entry_set, answer_id)
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
        let (form, answer_entry_set) = self.read_form_and_entry_set(form_id, &actor_ref).await?;

        let visible_answers = form.readable_entries(&answer_entry_set);

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
        let forms = self
            .active_form_repository
            .list(None, None)
            .await?
            .into_iter()
            .filter_map(|form| form.try_read(actor_ref.clone()).ok())
            .collect::<Vec<_>>();
        let answer_entry_sets = self
            .answer_entry_set_repository
            .list_read_by_forms(&forms)
            .await?;
        let form_by_id = forms
            .into_iter()
            .map(|form| (form.value().id().into_inner(), form))
            .collect::<HashMap<_, _>>();

        let visible_answers: Vec<(FormId, Allowed<AnswerEntry, Read>)> =
            stream::iter(answer_entry_sets)
                .then(|answer_entry_set| {
                    let form_by_id = &form_by_id;
                    async move {
                        let form_id = *answer_entry_set.form_id();
                        let Some(form) = form_by_id.get(&form_id.into_inner()) else {
                            return Ok::<_, Error>(Vec::new());
                        };

                        Ok(form
                            .readable_entries(&answer_entry_set)
                            .into_iter()
                            .map(|entry| (form_id, entry))
                            .collect::<Vec<_>>())
                    }
                })
                .try_collect::<Vec<Vec<_>>>()
                .await?
                .into_iter()
                .flatten()
                .collect();

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
        let (form, answer_entry_set) = self.read_form_and_entry_set(form_id, &actor_ref).await?;

        let form_answer = match title {
            Some(title) => {
                let form_update = self
                    .active_form_repository
                    .get(form_id)
                    .await?
                    .ok_or(FormNotFound)?
                    .into_update()
                    .try_update(actor_ref.clone())?;
                let set_update = self
                    .answer_entry_set_repository
                    .get_update(&form_update)
                    .await?
                    .ok_or(FormNotFound)?;
                let form_answer = form_update.change_entry_title(&set_update, answer_id, title)?;

                self.answer_entry_set_repository
                    .update_entry(&set_update, &form_answer)
                    .await?;

                let (form, answer_entry_set) =
                    self.read_form_and_entry_set(form_id, &actor_ref).await?;
                form.read_entry(&answer_entry_set, answer_id)
                    .map_err(|error| match error {
                        DomainError::NotFound => Error::from(AnswerNotFound),
                        error => Error::from(error),
                    })?
            }
            None => form
                .read_entry(&answer_entry_set, answer_id)
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
