use crate::{
    models::{ActiveFormWithLabels, AnswerDetails, CommentWithAuthor, CrossSearchOutput},
    user_reference_resolver::resolve_user_references,
};
use domain::repository::form::answer_entry_repository::AnswerEntryRepository;
use domain::repository::form::answer_label_repository::AnswerLabelRepository;
use domain::repository::form::comment_repository::CommentRepository;
use domain::repository::form::form_label_repository::FormLabelRepository;
use domain::repository::user_repository::UserRepository;
use domain::search::models::NumberOfRecords;
use domain::search::models::{NumberOfRecordsPerAggregate, Operation, UserSearchHit};
use domain::{
    account::models::AccountUser,
    auth::Actor,
    form::{
        answer::{AnswerAuthor, AnswerEntry, AnswerId},
        comment::Comment,
        models::ActiveForm,
    },
    pagination::{PageLimit, PageRequest},
    repository::{
        form::active_form_repository::ActiveFormRepository, search_repository::SearchRepository,
    },
    search::models::SearchableFieldsWithOperation,
    types::authorization_guard::{Allowed, AuthorizationGuard, Read},
};
use errors::Error;
use errors::usecase::UseCaseError::UserNotFound;
use futures::{StreamExt, TryStreamExt, stream, try_join};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Notify, mpsc::Receiver};
use tokio::time;

pub struct SearchUseCase<
    'a,
    SearchRepo: SearchRepository,
    FormRepo: ActiveFormRepository,
    FormAnswerLabelRepo: AnswerLabelRepository,
    FormLabelRepo: FormLabelRepository,
    UserRepo: UserRepository,
    AnswerEntryRepo: AnswerEntryRepository,
    CommentRepo: CommentRepository,
> {
    pub search_repository: &'a SearchRepo,
    pub active_form_repository: &'a FormRepo,
    pub form_answer_label_repository: &'a FormAnswerLabelRepo,
    pub form_label_repository: &'a FormLabelRepo,
    pub user_repository: &'a UserRepo,
    pub answer_entry_repository: &'a AnswerEntryRepo,
    pub comment_repository: &'a CommentRepo,
}

impl<
    R1: SearchRepository,
    R2: ActiveFormRepository,
    R3: AnswerLabelRepository,
    R4: FormLabelRepository,
    R5: UserRepository,
    R6: AnswerEntryRepository,
    R7: CommentRepository,
> SearchUseCase<'_, R1, R2, R3, R4, R5, R6, R7>
{
    /// 認可済みの `answer_entries` から `answer_id` の回答を探して返す。回答の公開範囲は
    /// `answer_entries` を取得した時点で各フォームの読み取りガードが検証済みのため、ここでは
    /// 同一性の照合だけを行う。
    fn read_visible_answer(
        answer_entries: &[Allowed<AnswerEntry, Read>],
        answer_id: AnswerId,
    ) -> Option<Allowed<AnswerEntry, Read>> {
        answer_entries
            .iter()
            .find(|entry| *entry.value().id() == answer_id)
            .cloned()
    }

    async fn list_all_form_guards(
        &self,
    ) -> Result<Vec<AuthorizationGuard<ActiveForm, Read>>, Error> {
        self.active_form_repository.list_all().await
    }

    async fn list_all_answer_entries(
        &self,
        forms: &[Allowed<ActiveForm, Read>],
    ) -> Result<Vec<Allowed<AnswerEntry, Read>>, Error> {
        let mut request = PageRequest::first(PageLimit::default_limit());
        let mut answers = Vec::new();

        loop {
            let page = self
                .answer_entry_repository
                .list_all(forms, request)
                .await?;
            let (items, next) = page.into_parts();
            answers.extend(items);

            let Some(next) = next else {
                break;
            };
            request = PageRequest::after(next, PageLimit::default_limit());
        }

        Ok(answers)
    }

    async fn visible_users(
        &self,
        actor: &Actor,
        users: Vec<UserSearchHit>,
    ) -> Result<Vec<AccountUser>, Error> {
        let user_ids = users
            .iter()
            .map(|user| user.user_id.into_inner())
            .collect::<Vec<_>>();

        let visible_users_by_id = self
            .user_repository
            .find_by_ids(user_ids)
            .await?
            .into_iter()
            .filter_map(|guard| {
                guard.try_read(actor.clone()).ok().map(|user| {
                    let user = user.into_inner();
                    (*user.id(), user)
                })
            })
            .collect::<HashMap<_, _>>();

        Ok(users
            .into_iter()
            .filter_map(|user| visible_users_by_id.get(&user.user_id).cloned())
            .collect())
    }

    async fn visible_form_with_labels(
        &self,
        actor: &Actor,
        form_id: domain::form::models::FormId,
    ) -> Result<Option<ActiveFormWithLabels>, Error> {
        let Some(form) = self.active_form_repository.get(form_id).await? else {
            return Ok(None);
        };
        let Ok(form) = form.try_read(actor.clone()) else {
            return Ok(None);
        };
        let labels = self
            .form_label_repository
            .fetch_labels_by_form_id(form_id)
            .await?
            .into_iter()
            .map(|label| {
                label
                    .try_read(actor.clone())
                    .map(|label| label.into_inner())
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Some(ActiveFormWithLabels {
            form: form.into_inner(),
            labels,
        }))
    }

    async fn answer_details(
        &self,
        account_user: &AccountUser,
        actor: &Actor,
        answer: Allowed<AnswerEntry, Read>,
    ) -> Result<AnswerDetails, Error> {
        let answer_id = *answer.id();
        let form_id = *answer.form_id();
        let labels = self
            .form_answer_label_repository
            .get_labels_for_answers_by_answer_id(answer_id)
            .await?
            .into_iter()
            .map(|label| {
                label
                    .try_read(actor.clone())
                    .map(|label| label.into_inner())
            })
            .collect::<Result<Vec<_>, _>>()?;
        let users = resolve_user_references(
            self.user_repository,
            account_user,
            answer
                .author()
                .authenticated_user_id()
                .into_iter()
                .collect(),
        )
        .await?;
        let author = match answer.author() {
            AnswerAuthor::AuthenticatedUser(user_id) => Actor::AccountUser(
                users
                    .get(user_id)
                    .cloned()
                    .ok_or(Error::from(UserNotFound))?,
            ),
            AnswerAuthor::Temporary(temporary_user) => {
                Actor::TemporaryAnswerAuthor(temporary_user.clone())
            }
        };

        Ok(AnswerDetails {
            form_id,
            form_answer: answer.into_inner(),
            author,
            labels,
        })
    }

    async fn comments_with_authors(
        &self,
        account_user: &AccountUser,
        comments: Vec<Comment>,
    ) -> Result<Vec<CommentWithAuthor>, Error> {
        let users = resolve_user_references(
            self.user_repository,
            account_user,
            comments
                .iter()
                .map(|comment| *comment.commented_by())
                .collect(),
        )
        .await?;

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

    pub async fn search_users(
        &self,
        actor: &AccountUser,
        query: String,
    ) -> Result<Vec<AccountUser>, Error> {
        let actor = Actor::from(actor.clone());
        let users = self.search_repository.search_users(&query).await?;

        self.visible_users(&actor, users).await
    }

    pub async fn cross_search(
        &self,
        account_user: &AccountUser,
        query: String,
    ) -> Result<CrossSearchOutput, Error> {
        let actor = Actor::from(account_user.clone());
        let (forms, users, label_for_forms, label_for_answers, answers, comments) = try_join!(
            self.search_repository.search_forms(&query),
            self.search_repository.search_users(&query),
            self.search_repository.search_labels_for_forms(&query),
            self.search_repository.search_labels_for_answers(&query),
            self.search_repository.search_answers(&query),
            self.search_repository.search_comments(&query)
        )?;

        let actor_ref = &actor;

        let visible_forms = stream::iter(forms)
            .then(
                |form| async move { self.visible_form_with_labels(actor_ref, form.form_id).await },
            )
            .try_filter_map(|visible| std::future::ready(Ok(visible)))
            .try_collect()
            .await?;

        let visible_users = self.visible_users(actor_ref, users).await?;

        let visible_label_for_forms = stream::iter(label_for_forms)
            .then(|label| async move {
                self.form_label_repository
                    .fetch_label(label.label_id)
                    .await
                    .map(|guard| {
                        guard.and_then(|guard| {
                            guard
                                .try_read(actor_ref.clone())
                                .ok()
                                .map(|label| label.into_inner())
                        })
                    })
            })
            .try_filter_map(|visible| std::future::ready(Ok(visible)))
            .try_collect()
            .await?;

        let visible_label_for_answers = stream::iter(label_for_answers)
            .then(|label| async move {
                self.form_answer_label_repository
                    .get_label_for_answers(label.label_id)
                    .await
                    .map(|guard| {
                        guard.and_then(|guard| {
                            guard
                                .try_read(actor_ref.clone())
                                .ok()
                                .map(|label| label.into_inner())
                        })
                    })
            })
            .try_filter_map(|visible| std::future::ready(Ok(visible)))
            .try_collect()
            .await?;

        let readable_forms = self
            .list_all_form_guards()
            .await?
            .into_iter()
            .filter_map(|form| form.try_read(actor_ref.clone()).ok())
            .collect::<Vec<_>>();
        let answer_entries = self.list_all_answer_entries(&readable_forms).await?;

        let visible_answers = stream::iter(answers)
            .then(|entry| {
                let answer_entries = &answer_entries;

                async move {
                    let Some(answer) = Self::read_visible_answer(answer_entries, entry.answer_id)
                    else {
                        return Ok::<_, Error>(None);
                    };

                    self.answer_details(account_user, actor_ref, answer)
                        .await
                        .map(Some)
                }
            })
            .try_filter_map(|visible| std::future::ready(Ok(visible)))
            .try_collect()
            .await?;

        let visible_comments: Vec<Comment> = stream::iter(comments)
            .then(|comment| {
                let answer_entries = &answer_entries;

                async move {
                    let Some(answer) = Self::read_visible_answer(answer_entries, comment.answer_id)
                    else {
                        return Ok::<_, Error>(None);
                    };

                    Ok::<_, Error>(
                        self.comment_repository
                            .find_by_answer(&answer)
                            .await?
                            .into_iter()
                            .find(|loaded| *loaded.value().comment_id() == comment.comment_id)
                            .map(|comment| comment.into_inner()),
                    )
                }
            })
            .try_filter_map(|visible| std::future::ready(Ok(visible)))
            .try_collect()
            .await?;
        let visible_comments = self
            .comments_with_authors(account_user, visible_comments)
            .await?;

        Ok(CrossSearchOutput {
            forms: visible_forms,
            users: visible_users,
            label_for_forms: visible_label_for_forms,
            label_for_answers: visible_label_for_answers,
            answers: visible_answers,
            comments: visible_comments,
        })
    }

    pub async fn start_sync(
        &self,
        receiver: Receiver<SearchableFieldsWithOperation>,
        shutdown_notifier: Arc<Notify>,
    ) -> Result<(), Error> {
        let mut receiver = receiver;
        loop {
            tokio::select! {
                _ = shutdown_notifier.notified() => {
                    break;
                },
                _ = async {
                    if let Some(data) = receiver.recv().await {
                        self.search_repository.sync_search_engine(&[data]).await?
                    }

                    Ok::<_, Error>(())
                } => {}
            }
        }

        Ok(())
    }

    pub async fn start_watch_out_of_sync(
        &self,
        shutdown_notifier: Arc<Notify>,
    ) -> Result<(), Error> {
        let mut interval = time::interval(Duration::from_secs(60));

        loop {
            tokio::select! {
                _ = shutdown_notifier.notified() => {
                    break
                },
                _ = interval.tick() => {
                    let search_engine_records = self.search_repository.fetch_search_engine_stats().await?;

                    let repository_records = NumberOfRecordsPerAggregate {
                        form_meta_data: NumberOfRecords(self.active_form_repository.size().await?),
                        real_answers: NumberOfRecords(
                            self.answer_entry_repository.size().await?,
                        ),
                        form_answer_comments: NumberOfRecords(
                            self.comment_repository.size().await?,
                        ),
                        label_for_form_answers: NumberOfRecords(
                            self.form_answer_label_repository.size().await?,
                        ),
                        label_for_forms: NumberOfRecords(self.form_label_repository.size().await?),
                        users: NumberOfRecords(self.user_repository.size().await?),
                    };

                    let sync_rate = search_engine_records.try_into_sync_rate(&repository_records)?;

                    if sync_rate.is_out_of_sync() {
                        let system = Actor::System;

                        let form_guards = self
                            .list_all_form_guards()
                            .await?
                            .into_iter()
                            .map(|guard| guard.try_read(system.clone()).map_err(Into::into))
                            .collect::<Result<Vec<_>, errors::Error>>()?;

                        let forms = form_guards
                            .iter()
                            .map(|form| {
                                Ok((
                                    domain::search::models::SearchableFields::FormMetaData(
                                        domain::search::models::FormMetaData {
                                            id: form.value().id().to_owned(),
                                            title: form.value().title().to_owned(),
                                            description: form.value().description().to_owned(),
                                        },
                                    ),
                                    Operation::Update,
                                ))
                            })
                            .collect::<Result<Vec<_>, errors::Error>>()?;

                        let answer_entries = self.list_all_answer_entries(&form_guards).await?;

                        let answers = answer_entries
                            .iter()
                            .flat_map(|entry| {
                                let entry = entry.value();
                                entry
                                    .contents()
                                    .iter()
                                    .map(|content| {
                                        (
                                            domain::search::models::SearchableFields::RealAnswers(
                                                domain::search::models::RealAnswers {
                                                    id: content.id,
                                                    answer_id: entry.id().to_owned(),
                                                    question_id: content.question_id,
                                                    answer: content.answer.to_owned(),
                                                },
                                            ),
                                            Operation::Update,
                                        )
                                    })
                                    .collect::<Vec<_>>()
                            })
                            .collect::<Vec<_>>();

                        let comments = stream::iter(answer_entries.clone())
                        .then(|answer| async move {
                            self.comment_repository
                                .find_by_answer(&answer)
                                .await
                                .map(|comments| {
                                    comments
                                        .into_iter()
                                        .map(|comment| comment.into_inner())
                                        .map(|comment| {
                                            (
                                                domain::search::models::SearchableFields::FormAnswerComments(
                                                    domain::search::models::FormAnswerComments {
                                                        id: comment.comment_id().to_owned(),
                                                        answer_id: comment.answer_id().to_owned(),
                                                        content: comment
                                                            .content()
                                                            .to_owned()
                                                            .into_inner()
                                                            .into_inner(),
                                                    },
                                                ),
                                                Operation::Update,
                                            )
                                        })
                                        .collect::<Vec<_>>()
                                })
                        })
                        .try_collect::<Vec<_>>()
                        .await?
                        .into_iter()
                        .flatten()
                        .collect::<Vec<_>>();

                        let labels_for_forms = self
                            .form_label_repository
                            .fetch_labels()
                            .await?
                            .into_iter()
                            .map(|guard| {
                                let label = guard.try_read(system.clone())?.into_inner();

                                Ok((
                                    domain::search::models::SearchableFields::LabelForForms(
                                        domain::search::models::LabelForForms {
                                            id: label.id().to_owned(),
                                            name: label.name().to_owned().into_inner().into_inner(),
                                        },
                                    ),
                                    Operation::Update,
                                ))
                            })
                            .collect::<Result<Vec<_>, errors::Error>>()?;

                        let labels_for_answers = self
                            .form_answer_label_repository
                            .get_labels_for_answers()
                            .await?
                            .into_iter()
                            .map(|guard| {
                                let label = guard.try_read(system.clone())?.into_inner();

                                Ok((
                                    domain::search::models::SearchableFields::LabelForFormAnswers(
                                        domain::search::models::LabelForFormAnswers {
                                            id: label.id().to_owned(),
                                            name: label.name().to_owned().into_inner(),
                                        },
                                    ),
                                    Operation::Update,
                                ))
                            })
                            .collect::<Result<Vec<_>, errors::Error>>()?;

                        let users = self
                            .user_repository
                            .fetch_all_users()
                            .await?
                            .into_iter()
                            .map(|guard| {
                                let user = guard.try_read(system.clone())?.into_inner();

                                Ok((
                                    domain::search::models::SearchableFields::Users(
                                        domain::search::models::Users {
                                            id: user.id().into_inner(),
                                            name: user.name().to_owned(),
                                        },
                                    ),
                                    Operation::Update,
                                ))
                            })
                            .collect::<Result<Vec<_>, errors::Error>>()?;

                        let data = forms
                            .into_iter()
                            .chain(answers)
                            .chain(comments)
                            .chain(labels_for_forms)
                            .chain(labels_for_answers)
                            .chain(users)
                            .collect::<Vec<_>>();

                        self.search_repository
                            .sync_search_engine(data.as_slice())
                            .await?;
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn initialize_search_engine(&self) -> Result<(), Error> {
        self.search_repository.initialize_search_engine().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::repositories::{
        InMemoryActiveFormRepository, InMemoryAnswerEntryRepository, InMemoryFormLabelRepository,
        InMemoryUserRepository,
    };
    use domain::{
        account::models::{Role, UserGroup, UserGroupName},
        form::{
            models::{AllowedUserGroups, FormDescription, FormSettings, FormTitle},
            question::{Question, QuestionSet},
        },
        repository::{
            form::{
                answer_label_repository::MockAnswerLabelRepository,
                comment_repository::MockCommentRepository,
            },
            search_repository::MockSearchRepository,
        },
        search::models::FormSearchHit,
    };
    use types::non_empty_vec::NonEmptyVec;
    use uuid::Uuid;

    fn form_restricted_to(title: &str, group: &UserGroup) -> ActiveForm {
        let question = Question::new_text(
            "body".to_string().try_into().unwrap(),
            0,
            "Body".to_string().try_into().unwrap(),
            None,
            true,
        )
        .unwrap();
        let questions =
            QuestionSet::try_new(NonEmptyVec::try_new(vec![question]).unwrap()).unwrap();

        ActiveForm::new(
            FormTitle::new(title.to_string().try_into().unwrap()),
            FormDescription::default(),
            questions,
        )
        .change_settings(
            FormSettings::new()
                .change_allowed_user_groups(AllowedUserGroups::new(vec![*group.id()])),
        )
    }

    #[tokio::test]
    async fn cross_search_excludes_only_form_hits_the_actor_cannot_read() {
        let member_group = UserGroup::new(UserGroupName::new(
            "members".to_string().try_into().unwrap(),
        ));
        let other_group =
            UserGroup::new(UserGroupName::new("other".to_string().try_into().unwrap()));
        let actor = AccountUser::with_groups(
            "viewer".to_string(),
            Uuid::from_u128(1).into(),
            Role::StandardUser,
            vec![member_group.clone()],
        );
        let hidden_form = form_restricted_to("hidden", &other_group);
        let readable_form = form_restricted_to("readable", &member_group);
        let hidden_form_id = *hidden_form.id();
        let readable_form_id = *readable_form.id();

        let mut search_repository = MockSearchRepository::new();
        search_repository.expect_search_forms().returning(move |_| {
            Ok(vec![
                FormSearchHit {
                    form_id: hidden_form_id,
                },
                FormSearchHit {
                    form_id: readable_form_id,
                },
            ])
        });
        search_repository
            .expect_search_users()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_labels_for_forms()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_labels_for_answers()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_answers()
            .returning(|_| Ok(vec![]));
        search_repository
            .expect_search_comments()
            .returning(|_| Ok(vec![]));

        let active_form_repository =
            InMemoryActiveFormRepository::new(vec![hidden_form, readable_form]);
        let answer_label_repository = MockAnswerLabelRepository::new();
        let form_label_repository = InMemoryFormLabelRepository;
        let user_repository = InMemoryUserRepository::default();
        let answer_entry_repository = InMemoryAnswerEntryRepository::default();
        let comment_repository = MockCommentRepository::new();
        let use_case = SearchUseCase {
            search_repository: &search_repository,
            active_form_repository: &active_form_repository,
            form_answer_label_repository: &answer_label_repository,
            form_label_repository: &form_label_repository,
            user_repository: &user_repository,
            answer_entry_repository: &answer_entry_repository,
            comment_repository: &comment_repository,
        };

        let output = use_case
            .cross_search(&actor, "form".to_string())
            .await
            .unwrap();

        assert_eq!(output.forms.len(), 1);
        assert_eq!(*output.forms[0].form.id(), readable_form_id);
    }
}
