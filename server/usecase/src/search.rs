use crate::{
    models::{
        ActiveFormWithLabels, AnswerDetails, CommentAuthor, CommentWithAuthor, CrossSearchComment,
        CrossSearchOutput, PublishedAnswerAuthor, PublishedAnswerEntry,
    },
    user_reference_resolver::resolve_user_references,
};
use domain::repository::form::answer_entry_repository::AnswerEntryRepository;
use domain::repository::form::answer_label_repository::AnswerLabelRepository;
use domain::repository::form::comment_thread_repository::CommentThreadRepository;
use domain::repository::form::form_label_repository::FormLabelRepository;
use domain::repository::user_repository::UserRepository;
use domain::{
    account::models::AccountUser,
    auth::Actor,
    form::{
        answer::{AnswerAuthor, AnswerAuthorDisclosure, AnswerEntry, AnswerId},
        comment::Comment,
        comment_thread::CommentThread,
        models::{ActiveForm, FormId},
    },
    pagination::{PageLimit, PageRequest},
    repository::{
        form::active_form_repository::ActiveFormRepository, search_repository::SearchRepository,
    },
    search::models::{
        AnswerSearchHit, AnswerTitleSearchDocument, FormAnswerComments, FormMetaData,
        LabelForFormAnswers, LabelForForms, NumberOfRecords, NumberOfRecordsPerAggregate,
        Operation, RealAnswers, SearchableFields, SearchableFieldsWithOperation, UserSearchHit,
        Users,
    },
    types::authorization_guard::{Allowed, AuthorizationGuard, Read},
};
use errors::Error;
use futures::{StreamExt, TryStreamExt, stream, try_join};
use std::{
    collections::{HashMap, HashSet},
    future::ready,
    iter::once,
    sync::Arc,
    time::Duration,
};
use tokio::sync::{Notify, mpsc::Receiver};
use tokio::time;

const SEARCH_DETAIL_FETCH_CONCURRENCY: usize = 10;

pub struct SearchUseCase<
    'a,
    SearchRepo: SearchRepository,
    FormRepo: ActiveFormRepository,
    FormAnswerLabelRepo: AnswerLabelRepository,
    FormLabelRepo: FormLabelRepository,
    UserRepo: UserRepository,
    AnswerEntryRepo: AnswerEntryRepository,
    CommentThreadRepo: CommentThreadRepository,
> {
    pub search_repository: &'a SearchRepo,
    pub active_form_repository: &'a FormRepo,
    pub form_answer_label_repository: &'a FormAnswerLabelRepo,
    pub form_label_repository: &'a FormLabelRepo,
    pub user_repository: &'a UserRepo,
    pub answer_entry_repository: &'a AnswerEntryRepo,
    pub comment_thread_repository: &'a CommentThreadRepo,
}

impl<
    R1: SearchRepository,
    R2: ActiveFormRepository,
    R3: AnswerLabelRepository,
    R4: FormLabelRepository,
    R5: UserRepository,
    R6: AnswerEntryRepository,
    R7: CommentThreadRepository,
> SearchUseCase<'_, R1, R2, R3, R4, R5, R6, R7>
{
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

    async fn visible_answer_entries_by_id(
        &self,
        actor: &Actor,
        answer_ids: Vec<AnswerId>,
    ) -> Result<HashMap<AnswerId, Allowed<AnswerEntry, Read>>, Error> {
        if answer_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let readable_forms = self
            .list_all_form_guards()
            .await?
            .into_iter()
            .filter_map(|form| form.try_read(actor.clone()).ok())
            .collect::<Vec<_>>();

        Ok(self
            .answer_entry_repository
            .find_by_ids(&readable_forms, answer_ids)
            .await?
            .into_iter()
            .map(|answer| (*answer.id(), answer))
            .collect())
    }

    async fn comment_thread_for_answer(
        &self,
        actor: &Actor,
        answer: Allowed<AnswerEntry, Read>,
    ) -> Result<Option<Allowed<CommentThread, Read>>, Error> {
        let Some(form) = self.active_form_repository.get(*answer.form_id()).await? else {
            return Ok(None);
        };
        let Ok(form) = form.try_read(actor.clone()) else {
            return Ok(None);
        };

        self.comment_thread_repository
            .get_with_comments_for_answer(&form, answer.into_inner())
            .await
            .map(Some)
            .or_else(|error| {
                if matches!(
                    error,
                    Error::Domain {
                        source: errors::domain::DomainError::Forbidden
                    }
                ) {
                    Ok(None)
                } else {
                    Err(error)
                }
            })
    }

    async fn comment_search_documents(
        &self,
        forms: &[Allowed<ActiveForm, Read>],
        answers: &[Allowed<AnswerEntry, Read>],
    ) -> Result<Vec<SearchableFieldsWithOperation>, Error> {
        stream::iter(answers.iter().cloned())
            .then(|answer| async move {
                let form = forms
                    .iter()
                    .find(|form| form.id() == answer.form_id())
                    .ok_or(errors::domain::DomainError::NotFound)?;
                let thread = self
                    .comment_thread_repository
                    .get_with_comments_for_answer(form, answer.into_inner())
                    .await?;
                Ok::<_, Error>(
                    thread
                        .comments()
                        .iter()
                        .map(|comment| {
                            (
                                SearchableFields::FormAnswerComments(FormAnswerComments {
                                    id: comment.comment_id().to_owned(),
                                    answer_id: comment.answer_id().to_owned(),
                                    content: comment.content().to_owned().into_inner().into_inner(),
                                }),
                                Operation::Update,
                            )
                        })
                        .collect::<Vec<_>>(),
                )
            })
            .try_collect::<Vec<_>>()
            .await
            .map(|documents| documents.into_iter().flatten().collect())
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
        form_id: FormId,
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
    ) -> Result<Option<AnswerDetails>, Error> {
        let answer_id = *answer.id();
        let form_id = *answer.form_id();
        let Some(form) = self.active_form_repository.get(form_id).await? else {
            return Ok(None);
        };
        let Ok(form) = form.try_read(actor.clone()) else {
            return Ok(None);
        };
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
        let author = match form.answer_settings().author_disclosure_for(actor) {
            AnswerAuthorDisclosure::Anonymous => PublishedAnswerAuthor::Anonymous,
            AnswerAuthorDisclosure::Disclosed => {
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
                match answer.author() {
                    AnswerAuthor::AuthenticatedUser(user_id) => {
                        let Some(user) = users.get(user_id).cloned() else {
                            return Ok(None);
                        };
                        PublishedAnswerAuthor::AuthenticatedUser(user)
                    }
                    AnswerAuthor::Temporary(temporary_user) => {
                        PublishedAnswerAuthor::Temporary(temporary_user.clone())
                    }
                    AnswerAuthor::ImportedFromRedmine(author) => {
                        PublishedAnswerAuthor::ImportedFromRedmine(author.clone())
                    }
                }
            }
        };

        Ok(Some(AnswerDetails {
            form_id,
            answer: PublishedAnswerEntry::new(answer.into_inner(), author),
            labels,
        }))
    }

    async fn visible_answer_details(
        &self,
        account_user: &AccountUser,
        actor: &Actor,
        hits: Vec<AnswerSearchHit>,
        visible_answers_by_id: &HashMap<AnswerId, Allowed<AnswerEntry, Read>>,
    ) -> Result<Vec<AnswerDetails>, Error> {
        stream::iter(hits)
            .map(|hit| async move {
                let Some(answer) = visible_answers_by_id.get(&hit.answer_id).cloned() else {
                    return Ok::<_, Error>(None);
                };

                self.answer_details(account_user, actor, answer).await
            })
            .buffered(SEARCH_DETAIL_FETCH_CONCURRENCY)
            .try_filter_map(|visible| ready(Ok(visible)))
            .try_collect()
            .await
    }

    async fn cross_search_comments_with_authors(
        &self,
        account_user: &AccountUser,
        comments: Vec<(FormId, Comment)>,
    ) -> Result<Vec<CrossSearchComment>, Error> {
        let users = resolve_user_references(
            self.user_repository,
            account_user,
            comments
                .iter()
                .filter_map(|(_, comment)| comment.commented_by().copied())
                .collect(),
        )
        .await?;

        Ok(comments
            .into_iter()
            .filter_map(|(form_id, comment)| {
                let commented_by = match comment.commented_by() {
                    Some(user_id) => users.get(user_id).cloned().map(CommentAuthor::Portal)?,
                    None => comment
                        .redmine_author()
                        .cloned()
                        .map(CommentAuthor::ImportedFromRedmine)?,
                };
                Some(CrossSearchComment {
                    form_id,
                    comment: CommentWithAuthor {
                        comment,
                        commented_by,
                    },
                })
            })
            .collect())
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

    pub async fn search_answers(
        &self,
        account_user: &AccountUser,
        query: String,
        form_id: Option<FormId>,
    ) -> Result<Vec<AnswerDetails>, Error> {
        let actor = Actor::from(account_user.clone());
        if let Some(form_id) = form_id {
            let Some(form) = self.active_form_repository.get(form_id).await? else {
                return Ok(Vec::new());
            };
            if form.try_read(actor.clone()).is_err() {
                return Ok(Vec::new());
            }
        }

        let hits = self
            .search_repository
            .search_answers(&query, form_id)
            .await?;
        let answer_ids = unique_answer_ids(hits.iter().map(|hit| hit.answer_id));
        let visible_answers_by_id = self
            .visible_answer_entries_by_id(&actor, answer_ids)
            .await?;

        self.visible_answer_details(account_user, &actor, hits, &visible_answers_by_id)
            .await
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
            self.search_repository.search_answers(&query, None),
            self.search_repository.search_comments(&query)
        )?;

        let actor_ref = &actor;
        let answer_ids = unique_answer_ids(
            answers
                .iter()
                .map(|hit| hit.answer_id)
                .chain(comments.iter().map(|hit| hit.answer_id)),
        );

        let visible_forms = stream::iter(forms)
            .map(|form| async move { self.visible_form_with_labels(actor_ref, form.form_id).await })
            .buffered(SEARCH_DETAIL_FETCH_CONCURRENCY)
            .try_filter_map(|visible| ready(Ok(visible)))
            .try_collect()
            .await?;

        let visible_users = self.visible_users(actor_ref, users).await?;

        let visible_label_for_forms = stream::iter(label_for_forms)
            .map(|label| async move {
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
            .buffered(SEARCH_DETAIL_FETCH_CONCURRENCY)
            .try_filter_map(|visible| ready(Ok(visible)))
            .try_collect()
            .await?;

        let visible_label_for_answers = stream::iter(label_for_answers)
            .map(|label| async move {
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
            .buffered(SEARCH_DETAIL_FETCH_CONCURRENCY)
            .try_filter_map(|visible| ready(Ok(visible)))
            .try_collect()
            .await?;

        let visible_answers_by_id = self
            .visible_answer_entries_by_id(actor_ref, answer_ids)
            .await?;
        let visible_answers = self
            .visible_answer_details(account_user, actor_ref, answers, &visible_answers_by_id)
            .await?;

        let visible_comments: Vec<(FormId, Comment)> = stream::iter(comments)
            .map(|comment| {
                let visible_answers_by_id = &visible_answers_by_id;

                async move {
                    let Some(answer) = visible_answers_by_id.get(&comment.answer_id).cloned()
                    else {
                        return Ok::<_, Error>(None);
                    };
                    let form_id = *answer.form_id();
                    let Some(thread) = self.comment_thread_for_answer(actor_ref, answer).await?
                    else {
                        return Ok::<_, Error>(None);
                    };

                    Ok::<_, Error>(
                        thread
                            .comments()
                            .iter()
                            .find(|loaded| *loaded.comment_id() == comment.comment_id)
                            .cloned()
                            .map(|comment| (form_id, comment)),
                    )
                }
            })
            .buffered(SEARCH_DETAIL_FETCH_CONCURRENCY)
            .try_filter_map(|visible| ready(Ok(visible)))
            .try_collect()
            .await?;
        let visible_comments = self
            .cross_search_comments_with_authors(account_user, visible_comments)
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
                        answers: NumberOfRecords(self.answer_entry_repository.size().await?),
                        real_answers: NumberOfRecords(
                            self.answer_entry_repository.content_size().await?,
                        ),
                        form_answer_comments: NumberOfRecords(
                            self.comment_thread_repository.size().await?,
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
                            .collect::<Result<Vec<_>, Error>>()?;

                        let forms = form_guards
                            .iter()
                            .map(|form| {
                                Ok((
                                    SearchableFields::FormMetaData(
                                        FormMetaData {
                                            id: form.value().id().to_owned(),
                                            title: form.value().title().to_owned(),
                                            description: form.value().description().to_owned(),
                                        },
                                    ),
                                    Operation::Update,
                                ))
                            })
                            .collect::<Result<Vec<_>, Error>>()?;

                        let answer_entries = self.list_all_answer_entries(&form_guards).await?;

                        let answer_documents = answer_search_documents(&answer_entries);

                        let comments = self
                            .comment_search_documents(&form_guards, &answer_entries)
                            .await?;

                        let labels_for_forms = self
                            .form_label_repository
                            .fetch_labels()
                            .await?
                            .into_iter()
                            .map(|guard| {
                                let label = guard.try_read(system.clone())?.into_inner();

                                Ok((
                                    SearchableFields::LabelForForms(
                                        LabelForForms {
                                            id: label.id().to_owned(),
                                            name: label.name().to_owned().into_inner().into_inner(),
                                        },
                                    ),
                                    Operation::Update,
                                ))
                            })
                            .collect::<Result<Vec<_>, Error>>()?;

                        let labels_for_answers = self
                            .form_answer_label_repository
                            .get_labels_for_answers()
                            .await?
                            .into_iter()
                            .map(|guard| {
                                let label = guard.try_read(system.clone())?.into_inner();

                                Ok((
                                    SearchableFields::LabelForFormAnswers(
                                        LabelForFormAnswers {
                                            id: label.id().to_owned(),
                                            name: label.name().to_owned().into_inner(),
                                        },
                                    ),
                                    Operation::Update,
                                ))
                            })
                            .collect::<Result<Vec<_>, Error>>()?;

                        let users = self
                            .user_repository
                            .fetch_all_users()
                            .await?
                            .into_iter()
                            .map(|guard| {
                                let user = guard.try_read(system.clone())?.into_inner();

                                Ok((
                                    SearchableFields::Users(
                                        Users {
                                            id: user.id().into_inner(),
                                            name: user.name().to_owned(),
                                        },
                                    ),
                                    Operation::Update,
                                ))
                            })
                            .collect::<Result<Vec<_>, Error>>()?;

                        let data = forms
                            .into_iter()
                            .chain(answer_documents)
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
        if self.search_repository.initialize_search_engine().await? {
            let system = Actor::System;
            let form_guards = self
                .list_all_form_guards()
                .await?
                .into_iter()
                .map(|guard| guard.try_read(system.clone()).map_err(Into::into))
                .collect::<Result<Vec<_>, Error>>()?;
            let answer_entries = self.list_all_answer_entries(&form_guards).await?;
            let documents = answer_search_documents(&answer_entries);
            let comments = self
                .comment_search_documents(&form_guards, &answer_entries)
                .await?;
            self.search_repository
                .sync_search_engine(&documents.into_iter().chain(comments).collect::<Vec<_>>())
                .await?;
        }

        Ok(())
    }
}

fn answer_search_documents(
    answer_entries: &[Allowed<AnswerEntry, Read>],
) -> Vec<SearchableFieldsWithOperation> {
    answer_entries
        .iter()
        .flat_map(|entry| {
            let entry = entry.value();
            once((
                SearchableFields::AnswerTitle(AnswerTitleSearchDocument {
                    id: *entry.id(),
                    form_id: *entry.form_id(),
                    title: entry.title().clone(),
                }),
                Operation::Update,
            ))
            .chain(entry.contents().iter().map(|content| {
                (
                    SearchableFields::RealAnswers(RealAnswers {
                        id: content.id,
                        answer_id: *entry.id(),
                        question_id: content.question_id,
                        answer: content.answer.to_owned(),
                    }),
                    Operation::Update,
                )
            }))
        })
        .collect()
}

fn unique_answer_ids(answer_ids: impl IntoIterator<Item = AnswerId>) -> Vec<AnswerId> {
    let mut seen = HashSet::new();

    answer_ids
        .into_iter()
        .filter(|answer_id| seen.insert(*answer_id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::repositories::{
        InMemoryActiveFormRepository, InMemoryAnswerEntryRepository, InMemoryFormLabelRepository,
        InMemoryUserRepository,
    };
    use chrono::Utc;
    use domain::{
        account::models::{Role, UserGroup, UserGroupName},
        form::{
            answer::{
                AnswerAuthor, AnswerAuthorPublicationPolicy, AnswerEntry, AnswerPublication,
                AnswerSettings, AnswerTitle, AnswerVisibility, FormAnswerContent,
                FormAnswerContentId, PostedAnswerContents,
            },
            comment::{Comment, CommentContent, CommentId},
            models::{AllowedUserGroups, FormDescription, FormSettings, FormTitle},
            question::{Question, QuestionSet},
        },
        repository::{
            form::{
                answer_entry_repository::MockAnswerEntryRepository,
                answer_label_repository::MockAnswerLabelRepository,
                comment_thread_repository::MockCommentThreadRepository,
            },
            search_repository::MockSearchRepository,
        },
        search::models::{AnswerSearchHit, CommentSearchHit, FormSearchHit},
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
            .returning(|_, _| Ok(vec![]));
        search_repository
            .expect_search_comments()
            .returning(|_| Ok(vec![]));

        let active_form_repository =
            InMemoryActiveFormRepository::new(vec![hidden_form, readable_form]);
        let mut answer_label_repository = MockAnswerLabelRepository::new();
        answer_label_repository
            .expect_get_labels_for_answers_by_answer_id()
            .returning(|_| Ok(vec![]));
        let form_label_repository = InMemoryFormLabelRepository;
        let user_repository = InMemoryUserRepository::default();
        let answer_entry_repository = InMemoryAnswerEntryRepository::default();
        let comment_repository = MockCommentThreadRepository::new();
        let use_case = SearchUseCase {
            search_repository: &search_repository,
            active_form_repository: &active_form_repository,
            form_answer_label_repository: &answer_label_repository,
            form_label_repository: &form_label_repository,
            user_repository: &user_repository,
            answer_entry_repository: &answer_entry_repository,
            comment_thread_repository: &comment_repository,
        };

        let output = use_case
            .cross_search(&actor, "form".to_string())
            .await
            .unwrap();

        assert_eq!(output.forms.len(), 1);
        assert_eq!(*output.forms[0].form.id(), readable_form_id);
    }

    #[tokio::test]
    async fn search_answers_excludes_unreadable_hits_and_preserves_hit_order_and_duplicates() {
        let member_group = UserGroup::new(UserGroupName::new(
            "members".to_string().try_into().unwrap(),
        ));
        let other_group =
            UserGroup::new(UserGroupName::new("other".to_string().try_into().unwrap()));
        let actor = AccountUser::with_groups(
            "viewer".to_string(),
            Uuid::from_u128(20).into(),
            Role::StandardUser,
            vec![member_group.clone()],
        );
        let answer_author = AccountUser::new(
            "answer author".to_string(),
            Uuid::from_u128(21).into(),
            Role::StandardUser,
        );
        let readable_form = form_restricted_to("readable answers", &member_group)
            .change_answer_settings(
                AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
            );
        let hidden_form = form_restricted_to("hidden answers", &other_group)
            .change_answer_settings(
                AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
            );
        let readable_form_id = *readable_form.id();
        let answer_for = |form: &ActiveForm| {
            let question_id = *form.questions().as_slice()[0].id();
            AnswerEntry::new(
                *form.id(),
                AnswerAuthor::AuthenticatedUser(*answer_author.id()),
                AnswerTitle::default(),
                PostedAnswerContents::try_new(
                    form.questions().as_slice(),
                    vec![FormAnswerContent {
                        id: FormAnswerContentId::from(Uuid::new_v4()),
                        question_id: question_id.into(),
                        answer: "body".to_string(),
                    }],
                )
                .unwrap(),
            )
        };
        let visible_answer_a = answer_for(&readable_form);
        let visible_answer_b = answer_for(&readable_form);
        let private_answer =
            answer_for(&readable_form).change_publication(AnswerPublication::PRIVATE);
        let hidden_answer = answer_for(&hidden_form);
        let visible_answer_a_id = *visible_answer_a.id();
        let visible_answer_b_id = *visible_answer_b.id();
        let private_answer_id = *private_answer.id();
        let hidden_answer_id = *hidden_answer.id();

        let mut search_repository = MockSearchRepository::new();
        search_repository
            .expect_search_answers()
            .withf(move |_, form_id| *form_id == Some(readable_form_id))
            .returning(move |_, _| {
                Ok(vec![
                    AnswerSearchHit {
                        answer_id: visible_answer_b_id,
                    },
                    AnswerSearchHit {
                        answer_id: visible_answer_b_id,
                    },
                    AnswerSearchHit {
                        answer_id: private_answer_id,
                    },
                    AnswerSearchHit {
                        answer_id: hidden_answer_id,
                    },
                    AnswerSearchHit {
                        answer_id: visible_answer_a_id,
                    },
                ])
            });

        let active_form_repository =
            InMemoryActiveFormRepository::new(vec![readable_form, hidden_form]);
        let mut answer_label_repository = MockAnswerLabelRepository::new();
        answer_label_repository
            .expect_get_labels_for_answers_by_answer_id()
            .times(3)
            .returning(|_| Ok(vec![]));
        let form_label_repository = InMemoryFormLabelRepository;
        let user_repository = InMemoryUserRepository::default();
        user_repository.save_user(answer_author.clone());
        let mut answer_entry_repository = MockAnswerEntryRepository::new();
        answer_entry_repository
            .expect_find_by_ids()
            .withf(move |_, answer_ids| {
                answer_ids
                    == &vec![
                        visible_answer_b_id,
                        private_answer_id,
                        hidden_answer_id,
                        visible_answer_a_id,
                    ]
            })
            .return_once(move |forms, _| {
                let form = forms
                    .iter()
                    .find(|form| form.id() == visible_answer_a.form_id())
                    .unwrap();

                Ok(vec![visible_answer_a, visible_answer_b, private_answer]
                    .into_iter()
                    .filter_map(|answer| form.read_entry(answer).ok())
                    .collect())
            });
        let comment_repository = MockCommentThreadRepository::new();
        let use_case = SearchUseCase {
            search_repository: &search_repository,
            active_form_repository: &active_form_repository,
            form_answer_label_repository: &answer_label_repository,
            form_label_repository: &form_label_repository,
            user_repository: &user_repository,
            answer_entry_repository: &answer_entry_repository,
            comment_thread_repository: &comment_repository,
        };

        let answers = use_case
            .search_answers(&actor, "answer".to_string(), Some(readable_form_id))
            .await
            .unwrap();

        let answer_ids = answers
            .iter()
            .map(|answer| answer.answer.id)
            .collect::<Vec<_>>();
        assert_eq!(
            answer_ids,
            vec![
                visible_answer_b_id,
                visible_answer_b_id,
                visible_answer_a_id
            ]
        );
    }

    #[tokio::test]
    async fn search_answers_keeps_an_answer_with_a_hidden_missing_author_as_anonymous() {
        let member_group = UserGroup::new(UserGroupName::new(
            "members".to_string().try_into().unwrap(),
        ));
        let actor = AccountUser::with_groups(
            "viewer".to_string(),
            Uuid::from_u128(20).into(),
            Role::StandardUser,
            vec![member_group.clone()],
        );
        let form = form_restricted_to("hidden author", &member_group).change_answer_settings(
            AnswerSettings::default()
                .change_visibility(AnswerVisibility::PUBLIC)
                .change_author_publication_policy(AnswerAuthorPublicationPolicy::Hide),
        );
        let form_id = *form.id();
        let question_id = *form.questions().as_slice()[0].id();
        let answer = AnswerEntry::new(
            form_id,
            AnswerAuthor::AuthenticatedUser(Uuid::from_u128(999).into()),
            AnswerTitle::default(),
            PostedAnswerContents::try_new(
                form.questions().as_slice(),
                vec![FormAnswerContent {
                    id: FormAnswerContentId::from(Uuid::new_v4()),
                    question_id: question_id.into(),
                    answer: "body".to_string(),
                }],
            )
            .unwrap(),
        );
        let answer_id = *answer.id();

        let mut search_repository = MockSearchRepository::new();
        search_repository
            .expect_search_answers()
            .return_once(move |_, _| Ok(vec![AnswerSearchHit { answer_id }]));
        let active_form_repository = InMemoryActiveFormRepository::new(vec![form]);
        let mut answer_label_repository = MockAnswerLabelRepository::new();
        answer_label_repository
            .expect_get_labels_for_answers_by_answer_id()
            .returning(|_| Ok(vec![]));
        let form_label_repository = InMemoryFormLabelRepository;
        let user_repository = InMemoryUserRepository::default();
        let answer_entry_repository = InMemoryAnswerEntryRepository::new(vec![answer]);
        let comment_repository = MockCommentThreadRepository::new();
        let use_case = SearchUseCase {
            search_repository: &search_repository,
            active_form_repository: &active_form_repository,
            form_answer_label_repository: &answer_label_repository,
            form_label_repository: &form_label_repository,
            user_repository: &user_repository,
            answer_entry_repository: &answer_entry_repository,
            comment_thread_repository: &comment_repository,
        };

        let answers = use_case
            .search_answers(&actor, "answer".to_string(), Some(form_id))
            .await
            .unwrap();

        assert_eq!(answers.len(), 1);
        assert_eq!(answers[0].answer.id, answer_id);
        assert!(matches!(
            answers[0].answer.author,
            PublishedAnswerAuthor::Anonymous
        ));
    }

    #[tokio::test]
    async fn search_answers_returns_empty_without_searching_for_a_missing_form() {
        let actor = AccountUser::new(
            "viewer".to_string(),
            Uuid::from_u128(20).into(),
            Role::StandardUser,
        );
        let missing_form_id = Uuid::from_u128(21).into();
        let search_repository = MockSearchRepository::new();
        let active_form_repository = InMemoryActiveFormRepository::default();
        let answer_label_repository = MockAnswerLabelRepository::new();
        let form_label_repository = InMemoryFormLabelRepository;
        let user_repository = InMemoryUserRepository::default();
        let answer_entry_repository = InMemoryAnswerEntryRepository::default();
        let comment_repository = MockCommentThreadRepository::new();
        let use_case = SearchUseCase {
            search_repository: &search_repository,
            active_form_repository: &active_form_repository,
            form_answer_label_repository: &answer_label_repository,
            form_label_repository: &form_label_repository,
            user_repository: &user_repository,
            answer_entry_repository: &answer_entry_repository,
            comment_thread_repository: &comment_repository,
        };

        let answers = use_case
            .search_answers(&actor, "answer".to_string(), Some(missing_form_id))
            .await
            .unwrap();

        assert!(answers.is_empty());
    }

    #[tokio::test]
    async fn search_answers_returns_empty_without_searching_for_an_unreadable_form() {
        let permitted_group = UserGroup::new(UserGroupName::new(
            "permitted".to_string().try_into().unwrap(),
        ));
        let actor = AccountUser::new(
            "viewer".to_string(),
            Uuid::from_u128(20).into(),
            Role::StandardUser,
        );
        let unreadable_form = form_restricted_to("unreadable", &permitted_group);
        let unreadable_form_id = *unreadable_form.id();
        let search_repository = MockSearchRepository::new();
        let active_form_repository = InMemoryActiveFormRepository::new(vec![unreadable_form]);
        let answer_label_repository = MockAnswerLabelRepository::new();
        let form_label_repository = InMemoryFormLabelRepository;
        let user_repository = InMemoryUserRepository::default();
        let answer_entry_repository = InMemoryAnswerEntryRepository::default();
        let comment_repository = MockCommentThreadRepository::new();
        let use_case = SearchUseCase {
            search_repository: &search_repository,
            active_form_repository: &active_form_repository,
            form_answer_label_repository: &answer_label_repository,
            form_label_repository: &form_label_repository,
            user_repository: &user_repository,
            answer_entry_repository: &answer_entry_repository,
            comment_thread_repository: &comment_repository,
        };

        let answers = use_case
            .search_answers(&actor, "answer".to_string(), Some(unreadable_form_id))
            .await
            .unwrap();

        assert!(answers.is_empty());
    }

    #[tokio::test]
    async fn cross_search_excludes_only_answer_hits_whose_author_is_missing() {
        let member_group = UserGroup::new(UserGroupName::new(
            "members".to_string().try_into().unwrap(),
        ));
        let actor = AccountUser::with_groups(
            "viewer".to_string(),
            Uuid::from_u128(1).into(),
            Role::StandardUser,
            vec![member_group.clone()],
        );
        let form = form_restricted_to("answers", &member_group).change_answer_settings(
            AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
        );
        let form_id = *form.id();
        let question_id = *form.questions().as_slice()[0].id();
        let answer_contents = || {
            PostedAnswerContents::try_new(
                form.questions().as_slice(),
                vec![FormAnswerContent {
                    id: FormAnswerContentId::from(Uuid::new_v4()),
                    question_id: question_id.into(),
                    answer: "body".to_string(),
                }],
            )
            .unwrap()
        };
        let missing_author_answer = AnswerEntry::new(
            form_id,
            AnswerAuthor::AuthenticatedUser(Uuid::from_u128(4).into()),
            AnswerTitle::default(),
            answer_contents(),
        );
        let visible_answer = AnswerEntry::new(
            form_id,
            AnswerAuthor::AuthenticatedUser(*actor.id()),
            AnswerTitle::default(),
            answer_contents(),
        );
        let missing_author_answer_id = *missing_author_answer.id();
        let visible_answer_id = *visible_answer.id();

        let mut search_repository = MockSearchRepository::new();
        search_repository
            .expect_search_forms()
            .returning(|_| Ok(vec![]));
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
            .returning(move |_, _| {
                Ok(vec![
                    AnswerSearchHit {
                        answer_id: missing_author_answer_id,
                    },
                    AnswerSearchHit {
                        answer_id: visible_answer_id,
                    },
                ])
            });
        search_repository
            .expect_search_comments()
            .returning(|_| Ok(vec![]));

        let active_form_repository = InMemoryActiveFormRepository::new(vec![form]);
        let mut answer_label_repository = MockAnswerLabelRepository::new();
        answer_label_repository
            .expect_get_labels_for_answers_by_answer_id()
            .returning(|_| Ok(vec![]));
        let form_label_repository = InMemoryFormLabelRepository;
        let user_repository = InMemoryUserRepository::default();
        user_repository.save_user(actor.clone());
        let answer_entry_repository =
            InMemoryAnswerEntryRepository::new(vec![missing_author_answer, visible_answer]);
        let comment_repository = MockCommentThreadRepository::new();
        let use_case = SearchUseCase {
            search_repository: &search_repository,
            active_form_repository: &active_form_repository,
            form_answer_label_repository: &answer_label_repository,
            form_label_repository: &form_label_repository,
            user_repository: &user_repository,
            answer_entry_repository: &answer_entry_repository,
            comment_thread_repository: &comment_repository,
        };

        let output = use_case
            .cross_search(&actor, "answer".to_string())
            .await
            .unwrap();

        assert_eq!(output.answers.len(), 1);
        assert_eq!(output.answers[0].answer.id, visible_answer_id);
    }

    #[tokio::test]
    async fn cross_search_excludes_only_comment_hits_whose_author_is_missing() {
        let member_group = UserGroup::new(UserGroupName::new(
            "members".to_string().try_into().unwrap(),
        ));
        let actor = AccountUser::with_groups(
            "viewer".to_string(),
            Uuid::from_u128(10).into(),
            Role::StandardUser,
            vec![member_group.clone()],
        );
        let form = form_restricted_to("comments", &member_group).change_answer_settings(
            AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
        );
        let form_id = *form.id();
        let question_id = *form.questions().as_slice()[0].id();
        let answer = AnswerEntry::new(
            *form.id(),
            AnswerAuthor::AuthenticatedUser(*actor.id()),
            AnswerTitle::default(),
            PostedAnswerContents::try_new(
                form.questions().as_slice(),
                vec![FormAnswerContent {
                    id: FormAnswerContentId::from(Uuid::new_v4()),
                    question_id: question_id.into(),
                    answer: "body".to_string(),
                }],
            )
            .unwrap(),
        );
        let answer_id = *answer.id();
        let first_comment_id = CommentId::from(Uuid::from_u128(11));
        let missing_author_comment_id = CommentId::from(Uuid::from_u128(12));
        let second_comment_id = CommentId::from(Uuid::from_u128(13));
        let comment = |comment_id, commented_by, content: &str| unsafe {
            Comment::from_raw_parts(
                answer_id,
                comment_id,
                CommentContent::new(content.to_string().try_into().unwrap()),
                Utc::now(),
                commented_by,
            )
        };
        let first_comment = comment(first_comment_id, *actor.id(), "first");
        let missing_author_comment = comment(
            missing_author_comment_id,
            Uuid::from_u128(14).into(),
            "missing author",
        );
        let second_comment = comment(second_comment_id, *actor.id(), "second");

        let mut search_repository = MockSearchRepository::new();
        search_repository
            .expect_search_forms()
            .returning(|_| Ok(vec![]));
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
            .returning(move |_, _| {
                Ok(vec![
                    AnswerSearchHit { answer_id },
                    AnswerSearchHit { answer_id },
                ])
            });
        search_repository
            .expect_search_comments()
            .returning(move |_| {
                Ok(vec![
                    CommentSearchHit {
                        comment_id: second_comment_id,
                        answer_id,
                    },
                    CommentSearchHit {
                        comment_id: missing_author_comment_id,
                        answer_id,
                    },
                    CommentSearchHit {
                        comment_id: first_comment_id,
                        answer_id,
                    },
                ])
            });

        let active_form_repository = InMemoryActiveFormRepository::new(vec![form]);
        let mut answer_label_repository = MockAnswerLabelRepository::new();
        answer_label_repository
            .expect_get_labels_for_answers_by_answer_id()
            .times(2)
            .returning(|_| Ok(vec![]));
        let form_label_repository = InMemoryFormLabelRepository;
        let user_repository = InMemoryUserRepository::default();
        user_repository.save_user(actor.clone());
        let mut answer_entry_repository = MockAnswerEntryRepository::new();
        answer_entry_repository
            .expect_find_by_ids()
            .withf(move |_, answer_ids| answer_ids == &vec![answer_id])
            .return_once(move |forms, _| {
                let form = forms
                    .iter()
                    .find(|form| form.id() == answer.form_id())
                    .unwrap();
                Ok(vec![form.read_entry(answer).unwrap()])
            });
        let stored_comments = vec![first_comment, missing_author_comment, second_comment];
        let mut comment_repository = MockCommentThreadRepository::new();
        comment_repository
            .expect_get_with_comments_for_answer()
            .returning(move |form, answer| {
                form.comment_thread_with_comments(answer, stored_comments.clone())
                    .map_err(Error::from)
            });
        let use_case = SearchUseCase {
            search_repository: &search_repository,
            active_form_repository: &active_form_repository,
            form_answer_label_repository: &answer_label_repository,
            form_label_repository: &form_label_repository,
            user_repository: &user_repository,
            answer_entry_repository: &answer_entry_repository,
            comment_thread_repository: &comment_repository,
        };

        let output = use_case
            .cross_search(&actor, "comment".to_string())
            .await
            .unwrap();

        let answer_ids = output
            .answers
            .iter()
            .map(|answer| answer.answer.id)
            .collect::<Vec<_>>();
        assert_eq!(answer_ids, vec![answer_id, answer_id]);

        let comment_ids = output
            .comments
            .iter()
            .map(|comment| *comment.comment.comment.comment_id())
            .collect::<Vec<_>>();
        assert_eq!(comment_ids, vec![second_comment_id, first_comment_id]);

        let form_ids = output
            .comments
            .iter()
            .map(|comment| comment.form_id)
            .collect::<Vec<_>>();
        assert_eq!(form_ids, vec![form_id, form_id]);
    }

    #[tokio::test]
    async fn cross_search_comments_keep_authorized_answers_parent_form_and_exclude_unavailable_or_unreadable_hits()
     {
        let member_group = UserGroup::new(UserGroupName::new(
            "members".to_string().try_into().unwrap(),
        ));
        let other_group =
            UserGroup::new(UserGroupName::new("other".to_string().try_into().unwrap()));
        let actor = AccountUser::with_groups(
            "viewer".to_string(),
            Uuid::from_u128(20).into(),
            Role::StandardUser,
            vec![member_group.clone()],
        );
        let form_a = form_restricted_to("comments a", &member_group).change_answer_settings(
            AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
        );
        let form_b = form_restricted_to("comments b", &member_group).change_answer_settings(
            AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
        );
        let unreadable_form = form_restricted_to("comments hidden", &other_group);
        let form_a_id = *form_a.id();
        let form_b_id = *form_b.id();
        let answer_author_id = Uuid::from_u128(28).into();
        let answer = |form: &ActiveForm| {
            AnswerEntry::new(
                *form.id(),
                AnswerAuthor::AuthenticatedUser(answer_author_id),
                AnswerTitle::default(),
                PostedAnswerContents::try_new(
                    form.questions().as_slice(),
                    vec![FormAnswerContent {
                        id: FormAnswerContentId::from(Uuid::new_v4()),
                        question_id: (*form.questions().as_slice()[0].id()).into(),
                        answer: "body".to_string(),
                    }],
                )
                .unwrap(),
            )
        };
        let answer_a = answer(&form_a);
        let answer_b = answer(&form_b);
        let private_answer = answer(&form_a).change_publication(AnswerPublication::PRIVATE);
        let unreadable_answer = answer(&unreadable_form);
        let answer_a_id = *answer_a.id();
        let answer_b_id = *answer_b.id();
        let private_answer_id = *private_answer.id();
        let unreadable_answer_id = *unreadable_answer.id();
        let unavailable_answer_id = AnswerId::from(Uuid::from_u128(21));
        let first_comment_id = CommentId::from(Uuid::from_u128(22));
        let missing_author_comment_id = CommentId::from(Uuid::from_u128(23));
        let second_comment_id = CommentId::from(Uuid::from_u128(24));
        let unavailable_comment_id = CommentId::from(Uuid::from_u128(25));
        let unreadable_comment_id = CommentId::from(Uuid::from_u128(27));
        let private_comment_id = CommentId::from(Uuid::from_u128(29));
        let comment = |answer_id, comment_id, commented_by, content: &str| unsafe {
            Comment::from_raw_parts(
                answer_id,
                comment_id,
                CommentContent::new(content.to_string().try_into().unwrap()),
                Utc::now(),
                commented_by,
            )
        };
        let first_comment = comment(answer_a_id, first_comment_id, *actor.id(), "first");
        let missing_author_comment = comment(
            answer_a_id,
            missing_author_comment_id,
            Uuid::from_u128(26).into(),
            "missing author",
        );
        let second_comment = comment(answer_b_id, second_comment_id, *actor.id(), "second");
        let unreadable_comment = comment(
            unreadable_answer_id,
            unreadable_comment_id,
            *actor.id(),
            "unreadable",
        );
        let private_comment = comment(
            private_answer_id,
            private_comment_id,
            *actor.id(),
            "private",
        );

        let mut search_repository = MockSearchRepository::new();
        search_repository
            .expect_search_forms()
            .returning(|_| Ok(vec![]));
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
            .returning(|_, _| Ok(vec![]));
        search_repository
            .expect_search_comments()
            .returning(move |_| {
                Ok(vec![
                    CommentSearchHit {
                        comment_id: second_comment_id,
                        answer_id: answer_b_id,
                    },
                    CommentSearchHit {
                        comment_id: unavailable_comment_id,
                        answer_id: unavailable_answer_id,
                    },
                    CommentSearchHit {
                        comment_id: unreadable_comment_id,
                        answer_id: unreadable_answer_id,
                    },
                    CommentSearchHit {
                        comment_id: private_comment_id,
                        answer_id: private_answer_id,
                    },
                    CommentSearchHit {
                        comment_id: missing_author_comment_id,
                        answer_id: answer_a_id,
                    },
                    CommentSearchHit {
                        comment_id: first_comment_id,
                        answer_id: answer_a_id,
                    },
                ])
            });

        let active_form_repository =
            InMemoryActiveFormRepository::new(vec![form_a, form_b, unreadable_form]);
        let answer_label_repository = MockAnswerLabelRepository::new();
        let form_label_repository = InMemoryFormLabelRepository;
        let user_repository = InMemoryUserRepository::default();
        user_repository.save_user(actor.clone());
        let answer_entry_repository = InMemoryAnswerEntryRepository::new(vec![
            answer_a,
            answer_b,
            private_answer,
            unreadable_answer,
        ]);
        let mut comment_repository = MockCommentThreadRepository::new();
        comment_repository
            .expect_get_with_comments_for_answer()
            .returning(move |form, answer| {
                let comments = match *answer.id() {
                    id if id == answer_a_id => {
                        vec![first_comment.clone(), missing_author_comment.clone()]
                    }
                    id if id == answer_b_id => vec![second_comment.clone()],
                    id if id == private_answer_id => vec![private_comment.clone()],
                    id if id == unreadable_answer_id => vec![unreadable_comment.clone()],
                    _ => vec![],
                };

                form.comment_thread_with_comments(answer, comments)
                    .map_err(Error::from)
            });
        let use_case = SearchUseCase {
            search_repository: &search_repository,
            active_form_repository: &active_form_repository,
            form_answer_label_repository: &answer_label_repository,
            form_label_repository: &form_label_repository,
            user_repository: &user_repository,
            answer_entry_repository: &answer_entry_repository,
            comment_thread_repository: &comment_repository,
        };

        let output = use_case
            .cross_search(&actor, "comment".to_string())
            .await
            .unwrap();

        let comment_parent_pairs = output
            .comments
            .iter()
            .map(|comment| (*comment.comment.comment.comment_id(), comment.form_id))
            .collect::<Vec<_>>();
        assert_eq!(
            comment_parent_pairs,
            vec![
                (second_comment_id, form_b_id),
                (first_comment_id, form_a_id)
            ]
        );
    }

    #[tokio::test]
    async fn cross_search_excludes_comment_hits_for_private_answer_visible_to_its_author() {
        let group = UserGroup::new(UserGroupName::new(
            "members".to_string().try_into().unwrap(),
        ));
        let actor = AccountUser::with_groups(
            "author".to_string(),
            Uuid::new_v4().into(),
            Role::StandardUser,
            vec![group.clone()],
        );
        let form = form_restricted_to("form", &group).change_answer_settings(
            AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
        );
        let answer = AnswerEntry::new(
            *form.id(),
            AnswerAuthor::AuthenticatedUser(*actor.id()),
            AnswerTitle::default(),
            PostedAnswerContents::try_new(
                form.questions().as_slice(),
                vec![FormAnswerContent {
                    id: FormAnswerContentId::new(),
                    question_id: (*form.questions().as_slice()[0].id()).into(),
                    answer: "answer".to_string(),
                }],
            )
            .unwrap(),
        )
        .change_publication(AnswerPublication::PRIVATE);
        let answer_id = *answer.id();
        let comment_id = CommentId::new();

        let mut search_repository = MockSearchRepository::new();
        search_repository
            .expect_search_forms()
            .returning(|_| Ok(vec![]));
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
            .returning(move |_, _| Ok(vec![AnswerSearchHit { answer_id }]));
        search_repository
            .expect_search_comments()
            .returning(move |_| {
                Ok(vec![CommentSearchHit {
                    comment_id,
                    answer_id,
                }])
            });

        let active_form_repository = InMemoryActiveFormRepository::new(vec![form]);
        let mut answer_label_repository = MockAnswerLabelRepository::new();
        answer_label_repository
            .expect_get_labels_for_answers_by_answer_id()
            .returning(|_| Ok(vec![]));
        let form_label_repository = InMemoryFormLabelRepository;
        let user_repository = InMemoryUserRepository::default();
        user_repository.save_user(actor.clone());
        let answer_entry_repository = InMemoryAnswerEntryRepository::new(vec![answer]);
        let mut comment_repository = MockCommentThreadRepository::new();
        comment_repository
            .expect_get_with_comments_for_answer()
            .returning(|form, answer| {
                form.comment_thread_with_comments(answer, Vec::new())
                    .map_err(Error::from)
            });
        let use_case = SearchUseCase {
            search_repository: &search_repository,
            active_form_repository: &active_form_repository,
            form_answer_label_repository: &answer_label_repository,
            form_label_repository: &form_label_repository,
            user_repository: &user_repository,
            answer_entry_repository: &answer_entry_repository,
            comment_thread_repository: &comment_repository,
        };

        let output = use_case
            .cross_search(&actor, "private".to_string())
            .await
            .unwrap();

        assert_eq!(output.answers.len(), 1);
        assert!(output.comments.is_empty());
    }
}
