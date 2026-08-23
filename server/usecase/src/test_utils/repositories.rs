use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{
    account::models::{
        AccountUser, DiscordAccountLink, DiscordUser, UserGroup, UserGroupId, UserId,
        UserPagePosition,
    },
    auth::Actor,
    form::{
        FormSubmissionRestriction, FormSubmissionRestrictionHistory, FormSubmissionRestrictionId,
        answer::{
            AnswerEntry, AnswerId, AnswerPagePosition, AnswerPublication, AnswerReference,
            AnswerRelation, AnswerStatus, AnswerStatusChange, AnswerStatusHistoryEntry,
            AnswerStatusHistoryPagePosition, AnswerTitleHistoryEntry,
            AnswerTitleHistoryPagePosition, ArchivedAnswerEntry, ReadableAnswerRelation,
        },
        models::{
            ActiveForm, ArchivedForm, ArchivedFormPagePosition, FormId, FormLabel, FormLabelId,
            FormPagePosition,
        },
    },
    notification::models::{
        Notification, NotificationId, NotificationPagePosition, NotificationPreference,
    },
    pagination::{Page, PageRequest},
    repository::{
        form::{
            active_form_repository::ActiveFormRepository,
            answer_entry_repository::AnswerEntryRepository,
            answer_relation_repository::AnswerRelationRepository,
            archived_form_repository::ArchivedFormRepository,
            form_label_repository::FormLabelRepository,
        },
        form_submission_restriction_repository::FormSubmissionRestrictionRepository,
        notification_repository::NotificationRepository,
        user_repository::UserRepository,
    },
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Delete, Read, Update},
};
use errors::Error;
use std::{collections::HashMap, sync::Mutex};
use uuid::Uuid;

use crate::forms::answer_relation::AnswerRelationUseCase;
use crate::forms::form::FormUseCase;

fn not_found_error(entity: &str, id: impl std::fmt::Display) -> Error {
    errors::domain::DomainError::InvalidEntity {
        message: format!("{entity} with id {id} not found"),
    }
    .into()
}

#[derive(Default)]
pub(crate) struct FormUseCaseTestRepositories {
    pub(crate) active_form_repository: InMemoryActiveFormRepository,
    pub(crate) archived_form_repository: InMemoryArchivedFormRepository,
    pub(crate) notification_repository: InMemoryNotificationRepository,
    pub(crate) form_label_repository: InMemoryFormLabelRepository,
    pub(crate) answer_entry_repository: InMemoryAnswerEntryRepository,
    pub(crate) answer_relation_repository: InMemoryAnswerRelationRepository,
    pub(crate) user_repository: InMemoryUserRepository,
    pub(crate) form_submission_restriction_repository: InMemoryFormSubmissionRestrictionRepository,
}

impl FormUseCaseTestRepositories {
    pub(crate) fn with_active_forms(forms: Vec<ActiveForm>) -> Self {
        Self {
            active_form_repository: InMemoryActiveFormRepository::new(forms),
            ..Self::default()
        }
    }

    pub(crate) fn form_use_case(
        &self,
    ) -> FormUseCase<
        '_,
        InMemoryActiveFormRepository,
        InMemoryArchivedFormRepository,
        InMemoryNotificationRepository,
        InMemoryFormLabelRepository,
        InMemoryAnswerEntryRepository,
        InMemoryUserRepository,
    > {
        FormUseCase {
            active_form_repository: &self.active_form_repository,
            archived_form_repository: &self.archived_form_repository,
            notification_repository: &self.notification_repository,
            form_label_repository: &self.form_label_repository,
            answer_entry_repository: &self.answer_entry_repository,
            user_repository: &self.user_repository,
            application_event_publisher: None,
        }
    }

    pub(crate) fn answer_relation_use_case(
        &self,
    ) -> AnswerRelationUseCase<
        '_,
        InMemoryActiveFormRepository,
        InMemoryArchivedFormRepository,
        InMemoryAnswerEntryRepository,
        InMemoryAnswerRelationRepository,
    > {
        AnswerRelationUseCase {
            active_form_repository: &self.active_form_repository,
            archived_form_repository: &self.archived_form_repository,
            answer_entry_repository: &self.answer_entry_repository,
            answer_relation_repository: &self.answer_relation_repository,
        }
    }
}

#[derive(Default)]
pub(crate) struct InMemoryActiveFormRepository {
    forms: Mutex<Vec<ActiveForm>>,
}

impl InMemoryActiveFormRepository {
    pub(crate) fn new(forms: Vec<ActiveForm>) -> Self {
        Self {
            forms: Mutex::new(forms),
        }
    }

    fn save_form(&self, form: ActiveForm) {
        let mut forms = self.forms.lock().unwrap();
        if let Some(stored_form) = forms.iter_mut().find(|stored| *stored.id() == *form.id()) {
            *stored_form = form;
        } else {
            forms.push(form);
        }
    }

    fn find_form(&self, id: FormId) -> Option<ActiveForm> {
        self.forms
            .lock()
            .unwrap()
            .iter()
            .find(|form| *form.id() == id)
            .cloned()
    }

    pub(crate) fn remove_form(&self, id: FormId) {
        self.forms.lock().unwrap().retain(|form| *form.id() != id);
    }
}

#[async_trait]
impl ActiveFormRepository for InMemoryActiveFormRepository {
    async fn create(
        &self,
        _actor: &AccountUser,
        form: Allowed<ActiveForm, Create>,
    ) -> Result<(), Error> {
        self.save_form(form.into_inner());
        Ok(())
    }

    async fn list(
        &self,
        request: PageRequest<FormPagePosition>,
    ) -> Result<Page<AuthorizationGuard<ActiveForm, Read>, FormPagePosition>, Error> {
        let mut forms = self
            .forms
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        forms.sort_by_key(|form| form.id().into_inner());

        if let Some(position) = request.after_position() {
            forms.retain(|form| *form.id() > position.last_form_id());
        }

        let page = Page::from_overfetched_items(forms, request.limit(), |form| {
            FormPagePosition::new(*form.id())
        });
        let (forms, next) = page.into_parts();

        Ok(Page::new(
            forms.into_iter().map(AuthorizationGuard::from).collect(),
            next,
        ))
    }

    async fn list_all(&self) -> Result<Vec<AuthorizationGuard<ActiveForm, Read>>, Error> {
        let mut forms = self
            .forms
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        forms.sort_by_key(|form| form.id().into_inner());

        Ok(forms.into_iter().map(AuthorizationGuard::from).collect())
    }

    async fn get(&self, id: FormId) -> Result<Option<AuthorizationGuard<ActiveForm, Read>>, Error> {
        Ok(self.find_form(id).map(AuthorizationGuard::from))
    }

    async fn update_form(
        &self,
        _actor: &AccountUser,
        updated_form: Allowed<ActiveForm, Update>,
    ) -> Result<(), Error> {
        let form = updated_form.into_inner();
        let mut forms = self.forms.lock().unwrap();
        if let Some(stored_form) = forms.iter_mut().find(|stored| *stored.id() == *form.id()) {
            *stored_form = form;
            Ok(())
        } else {
            Err(not_found_error("ActiveForm", form.id()))
        }
    }

    async fn size(&self) -> Result<u32, Error> {
        Ok(self.forms.lock().unwrap().len() as u32)
    }
}

#[derive(Default)]
pub(crate) struct InMemoryFormLabelRepository;

#[async_trait]
impl FormLabelRepository for InMemoryFormLabelRepository {
    async fn create_label_for_forms(
        &self,
        _label: Allowed<FormLabel, Create>,
    ) -> Result<(), Error> {
        Ok(())
    }

    async fn fetch_labels(&self) -> Result<Vec<AuthorizationGuard<FormLabel, Read>>, Error> {
        Ok(vec![])
    }

    async fn fetch_labels_by_ids(
        &self,
        _ids: Vec<FormLabelId>,
    ) -> Result<Vec<AuthorizationGuard<FormLabel, Read>>, Error> {
        Ok(vec![])
    }

    async fn fetch_label(
        &self,
        _id: FormLabelId,
    ) -> Result<Option<AuthorizationGuard<FormLabel, Read>>, Error> {
        Ok(None)
    }

    async fn delete_label_for_forms(
        &self,
        _label: Allowed<FormLabel, Delete>,
    ) -> Result<(), Error> {
        Ok(())
    }

    async fn edit_label_for_forms(
        &self,
        _id: FormLabelId,
        _label: Allowed<FormLabel, Update>,
    ) -> Result<(), Error> {
        Ok(())
    }

    async fn fetch_labels_by_form_id(
        &self,
        _form_id: FormId,
    ) -> Result<Vec<AuthorizationGuard<FormLabel, Read>>, Error> {
        Ok(vec![])
    }

    async fn size(&self) -> Result<u32, Error> {
        Ok(0)
    }
}

#[derive(Default)]
pub(crate) struct InMemoryAnswerEntryRepository {
    answers: Mutex<Vec<AnswerEntry>>,
}

impl InMemoryAnswerEntryRepository {
    pub(crate) fn new(answers: Vec<AnswerEntry>) -> Self {
        Self {
            answers: Mutex::new(answers),
        }
    }
}

#[async_trait]
impl AnswerEntryRepository for InMemoryAnswerEntryRepository {
    async fn get(
        &self,
        _form: &Allowed<ActiveForm, Read>,
        answer_id: AnswerId,
    ) -> Result<Option<Allowed<AnswerEntry, Read>>, Error> {
        Ok(self
            .answers
            .lock()
            .unwrap()
            .iter()
            .find(|answer| *answer.id() == answer_id)
            .cloned()
            .map(|answer| _form.read_entry(answer))
            .transpose()?)
    }

    async fn find_by_ids(
        &self,
        forms: &[Allowed<ActiveForm, Read>],
        answer_ids: Vec<AnswerId>,
    ) -> Result<Vec<Allowed<AnswerEntry, Read>>, Error> {
        let forms_by_id = forms
            .iter()
            .map(|form| (form.id().into_inner(), form))
            .collect::<HashMap<_, _>>();

        Ok(self
            .answers
            .lock()
            .unwrap()
            .iter()
            .filter(|answer| answer_ids.contains(answer.id()))
            .cloned()
            .filter_map(|answer| {
                forms_by_id
                    .get(&answer.form_id().into_inner())
                    .and_then(|form| form.read_entry(answer).ok())
            })
            .collect())
    }

    async fn list_by_form(
        &self,
        form: &Allowed<ActiveForm, Read>,
        request: PageRequest<AnswerPagePosition>,
        status: Option<AnswerStatus>,
    ) -> Result<Page<Allowed<AnswerEntry, Read>, AnswerPagePosition>, Error> {
        let mut answers = self
            .answers
            .lock()
            .unwrap()
            .iter()
            .filter(|answer| answer.form_id() == form.id())
            .filter(|answer| status.is_none_or(|status| *answer.status() == status))
            .cloned()
            .filter_map(|answer| form.read_entry(answer).ok())
            .collect::<Vec<_>>();
        answers.sort_by(|left, right| {
            right
                .timestamp()
                .cmp(left.timestamp())
                .then_with(|| right.id().into_inner().cmp(&left.id().into_inner()))
        });

        if let Some(position) = request.after_position() {
            answers.retain(|answer| position.is_followed_by(*answer.timestamp(), *answer.id()));
        }

        Ok(Page::from_overfetched_items(
            answers,
            request.limit(),
            |answer| AnswerPagePosition::new(*answer.timestamp(), *answer.id()),
        ))
    }

    async fn list_all(
        &self,
        forms: &[Allowed<ActiveForm, Read>],
        request: PageRequest<AnswerPagePosition>,
        status: Option<AnswerStatus>,
    ) -> Result<Page<Allowed<AnswerEntry, Read>, AnswerPagePosition>, Error> {
        let forms_by_id = forms
            .iter()
            .map(|form| (form.id().into_inner(), form))
            .collect::<HashMap<_, _>>();
        let mut answers = self
            .answers
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .filter(|answer| status.is_none_or(|status| *answer.status() == status))
            .filter_map(|answer| {
                forms_by_id
                    .get(&answer.form_id().into_inner())
                    .and_then(|form| form.read_entry(answer).ok())
            })
            .collect::<Vec<_>>();
        answers.sort_by(|left, right| {
            right
                .timestamp()
                .cmp(left.timestamp())
                .then_with(|| right.id().into_inner().cmp(&left.id().into_inner()))
        });

        if let Some(position) = request.after_position() {
            answers.retain(|answer| position.is_followed_by(*answer.timestamp(), *answer.id()));
        }

        Ok(Page::from_overfetched_items(
            answers,
            request.limit(),
            |answer| AnswerPagePosition::new(*answer.timestamp(), *answer.id()),
        ))
    }

    async fn post(
        &self,
        _form: &Allowed<ActiveForm, Read>,
        answer_entry: &Allowed<AnswerEntry, Create>,
    ) -> Result<(), Error> {
        self.answers
            .lock()
            .unwrap()
            .push(answer_entry.value().clone());
        Ok(())
    }

    async fn update(
        &self,
        _form: &Allowed<ActiveForm, Update>,
        answer_entry: &Allowed<AnswerEntry, Update>,
    ) -> Result<Option<AnswerStatusChange>, Error> {
        let mut answers = self.answers.lock().unwrap();
        if let Some(stored_answer) = answers
            .iter_mut()
            .find(|stored| *stored.id() == *answer_entry.id())
        {
            let status_change =
                AnswerStatusChange::new(*stored_answer.status(), *answer_entry.status());
            *stored_answer = answer_entry.value().clone();
            Ok(status_change)
        } else {
            Err(not_found_error("AnswerEntry", answer_entry.id()))
        }
    }

    async fn history(
        &self,
        _answer: &Allowed<AnswerEntry, Read>,
        _request: PageRequest<AnswerStatusHistoryPagePosition>,
    ) -> Result<Page<Allowed<AnswerStatusHistoryEntry, Read>, AnswerStatusHistoryPagePosition>, Error>
    {
        Ok(Page::new(Vec::new(), None))
    }

    async fn title_history(
        &self,
        _answer: &Allowed<AnswerEntry, Read>,
        _request: PageRequest<AnswerTitleHistoryPagePosition>,
    ) -> Result<Page<Allowed<AnswerTitleHistoryEntry, Read>, AnswerTitleHistoryPagePosition>, Error>
    {
        Ok(Page::new(Vec::new(), None))
    }

    async fn size(&self) -> Result<u32, Error> {
        Ok(self.answers.lock().unwrap().len() as u32)
    }

    async fn content_size(&self) -> Result<u32, Error> {
        Ok(self
            .answers
            .lock()
            .unwrap()
            .iter()
            .map(|answer| answer.contents().len() as u32)
            .sum())
    }
}

#[cfg(test)]
mod answer_entry_repository_tests {
    use chrono::{TimeZone, Utc};
    use domain::{
        account::models::Role,
        auth::Actor,
        form::{
            answer::{AnswerAuthor, AnswerStatus, AnswerTitle},
            models::{AnswerSettings, AnswerVisibility, FormDescription, FormTitle, QuestionSet},
            question::Question,
        },
        pagination::PageLimit,
        types::authorization_guard::AuthorizationGuard,
    };
    use types::non_empty_vec::NonEmptyVec;
    use uuid::Uuid;

    use super::*;

    fn active_form(title: &str) -> ActiveForm {
        let question = Question::new_text(
            "body".to_string().try_into().unwrap(),
            0,
            "Body".to_string().try_into().unwrap(),
            None,
            true,
        )
        .unwrap();

        ActiveForm::new(
            FormTitle::new(title.to_string().try_into().unwrap()),
            FormDescription::new("description".to_string()),
            QuestionSet::try_new(NonEmptyVec::try_new(vec![question]).unwrap()).unwrap(),
        )
    }

    fn answer(form: &ActiveForm, id: u128, timestamp: chrono::DateTime<Utc>) -> AnswerEntry {
        unsafe {
            AnswerEntry::from_raw_parts(
                Uuid::from_u128(id).into(),
                *form.id(),
                AnswerAuthor::AuthenticatedUser(Uuid::from_u128(id + 100).into()),
                timestamp,
                AnswerTitle::default(),
                AnswerPublication::PUBLIC,
                Vec::new(),
            )
        }
    }

    fn answer_with_status(
        form: &ActiveForm,
        id: u128,
        timestamp: chrono::DateTime<Utc>,
        status: AnswerStatus,
    ) -> AnswerEntry {
        unsafe {
            AnswerEntry::from_raw_parts_with_status_and_redmine_reference(
                Uuid::from_u128(id).into(),
                *form.id(),
                AnswerAuthor::AuthenticatedUser(Uuid::from_u128(id + 100).into()),
                timestamp,
                AnswerTitle::default(),
                AnswerPublication::PUBLIC,
                status,
                Vec::new(),
                None,
            )
        }
    }

    fn ids(entries: Vec<Allowed<AnswerEntry, Read>>) -> Vec<Uuid> {
        entries
            .into_iter()
            .map(|entry| entry.id().into_inner())
            .collect()
    }

    #[tokio::test]
    async fn list_filters_answers_by_status_before_paginating() {
        let form = active_form("answers");
        let timestamp = Utc.with_ymd_and_hms(2026, 8, 3, 12, 0, 0).unwrap();
        let repository = InMemoryAnswerEntryRepository::new(vec![
            answer_with_status(&form, 1, timestamp, AnswerStatus::COMPLETED),
            answer_with_status(
                &form,
                2,
                timestamp - chrono::TimeDelta::seconds(1),
                AnswerStatus::IN_PROGRESS,
            ),
            answer_with_status(
                &form,
                3,
                timestamp - chrono::TimeDelta::seconds(2),
                AnswerStatus::IN_PROGRESS,
            ),
        ]);
        let form = AuthorizationGuard::from(form)
            .try_read(Actor::System)
            .unwrap();

        let page = repository
            .list_by_form(
                &form,
                PageRequest::first(PageLimit::try_new(1).unwrap()),
                Some(AnswerStatus::IN_PROGRESS),
            )
            .await
            .unwrap();
        let (entries, next) = page.into_parts();
        assert_eq!(ids(entries), vec![Uuid::from_u128(2)]);
        assert!(next.is_some());
    }

    #[tokio::test]
    async fn list_all_preserves_global_order_across_pages() {
        let first_form = active_form("first");
        let second_form = active_form("second");
        let timestamp = Utc.with_ymd_and_hms(2026, 8, 3, 12, 0, 0).unwrap();
        let older_timestamp = Utc.with_ymd_and_hms(2026, 8, 3, 11, 59, 59).unwrap();
        let repository = InMemoryAnswerEntryRepository::new(vec![
            answer(&first_form, 1, timestamp),
            answer(&second_form, 3, timestamp),
            answer(&first_form, 4, older_timestamp),
            answer(&second_form, 2, timestamp),
        ]);
        let forms = [first_form, second_form].map(|form| {
            AuthorizationGuard::from(form)
                .try_read(Actor::System)
                .unwrap()
        });
        let limit = PageLimit::try_new(2).unwrap();

        let first_page = repository
            .list_all(&forms, PageRequest::first(limit), None)
            .await
            .unwrap();
        let (first_entries, next) = first_page.into_parts();
        let second_page = repository
            .list_all(&forms, PageRequest::after(next.unwrap(), limit), None)
            .await
            .unwrap();
        let (second_entries, next) = second_page.into_parts();

        assert_eq!(next, None);
        assert_eq!(
            ids(first_entries.into_iter().chain(second_entries).collect()),
            vec![
                Uuid::from_u128(3),
                Uuid::from_u128(2),
                Uuid::from_u128(1),
                Uuid::from_u128(4),
            ],
        );
    }

    #[tokio::test]
    async fn list_all_omits_private_answers_for_standard_user() {
        let form = active_form("answers").change_answer_settings(
            AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
        );
        let timestamp = Utc.with_ymd_and_hms(2026, 8, 3, 12, 0, 0).unwrap();
        let private_answer =
            answer(&form, 2, timestamp).change_publication(AnswerPublication::PRIVATE);
        let public_answer = answer(
            &form,
            1,
            Utc.with_ymd_and_hms(2026, 8, 3, 11, 59, 59).unwrap(),
        );
        let repository = InMemoryAnswerEntryRepository::new(vec![private_answer, public_answer]);
        let reader = AccountUser::new(
            "reader".to_string(),
            Uuid::from_u128(999).into(),
            Role::StandardUser,
        );
        let form = AuthorizationGuard::from(form)
            .try_read(Actor::from(reader))
            .unwrap();

        let page = repository
            .list_all(
                &[form],
                PageRequest::first(PageLimit::try_new(10).unwrap()),
                None,
            )
            .await
            .unwrap();

        assert_eq!(ids(page.into_items()), vec![Uuid::from_u128(1)]);
    }

    #[tokio::test]
    async fn list_all_scans_past_private_answers_to_fill_a_small_page() {
        let form = active_form("answers").change_answer_settings(
            AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
        );
        let timestamp = Utc.with_ymd_and_hms(2026, 8, 3, 12, 0, 0).unwrap();
        let repository = InMemoryAnswerEntryRepository::new(vec![
            answer(&form, 3, timestamp).change_publication(AnswerPublication::PRIVATE),
            answer(&form, 2, timestamp - chrono::TimeDelta::seconds(1))
                .change_publication(AnswerPublication::PRIVATE),
            answer(&form, 1, timestamp - chrono::TimeDelta::seconds(2)),
        ]);
        let reader = AccountUser::new(
            "reader".to_string(),
            Uuid::from_u128(999).into(),
            Role::StandardUser,
        );
        let form = AuthorizationGuard::from(form)
            .try_read(Actor::from(reader))
            .unwrap();

        let page = repository
            .list_all(
                &[form],
                PageRequest::first(PageLimit::try_new(2).unwrap()),
                None,
            )
            .await
            .unwrap();

        let (entries, next) = page.into_parts();
        assert_eq!(ids(entries), vec![Uuid::from_u128(1)]);
        assert_eq!(next, None);
    }

    #[tokio::test]
    async fn list_by_form_scans_past_private_answers_to_fill_a_small_page() {
        let form = active_form("answers").change_answer_settings(
            AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
        );
        let timestamp = Utc.with_ymd_and_hms(2026, 8, 3, 12, 0, 0).unwrap();
        let repository = InMemoryAnswerEntryRepository::new(vec![
            answer(&form, 3, timestamp).change_publication(AnswerPublication::PRIVATE),
            answer(&form, 2, timestamp - chrono::TimeDelta::seconds(1))
                .change_publication(AnswerPublication::PRIVATE),
            answer(&form, 1, timestamp - chrono::TimeDelta::seconds(2)),
        ]);
        let reader = AccountUser::new(
            "reader".to_string(),
            Uuid::from_u128(999).into(),
            Role::StandardUser,
        );
        let form = AuthorizationGuard::from(form)
            .try_read(Actor::from(reader))
            .unwrap();

        let page = repository
            .list_by_form(
                &form,
                PageRequest::first(PageLimit::try_new(2).unwrap()),
                None,
            )
            .await
            .unwrap();

        let (entries, next) = page.into_parts();
        assert_eq!(ids(entries), vec![Uuid::from_u128(1)]);
        assert_eq!(next, None);
    }

    #[tokio::test]
    async fn list_by_form_preserves_order_across_same_timestamp_boundary() {
        let form = active_form("target");
        let other_form = active_form("other");
        let timestamp = Utc.with_ymd_and_hms(2026, 8, 3, 12, 0, 0).unwrap();
        let repository = InMemoryAnswerEntryRepository::new(vec![
            answer(&form, 1, timestamp),
            answer(&other_form, 4, timestamp + chrono::TimeDelta::seconds(1)),
            answer(&form, 3, timestamp),
            answer(&form, 2, timestamp),
        ]);
        let form = AuthorizationGuard::from(form)
            .try_read(Actor::System)
            .unwrap();
        let limit = PageLimit::try_new(2).unwrap();

        let first_page = repository
            .list_by_form(&form, PageRequest::first(limit), None)
            .await
            .unwrap();
        let (first_entries, next) = first_page.into_parts();
        let second_page = repository
            .list_by_form(&form, PageRequest::after(next.unwrap(), limit), None)
            .await
            .unwrap();
        let (second_entries, next) = second_page.into_parts();

        assert_eq!(next, None);
        assert_eq!(
            ids(first_entries.into_iter().chain(second_entries).collect()),
            vec![Uuid::from_u128(3), Uuid::from_u128(2), Uuid::from_u128(1)],
        );
    }
}

#[derive(Default)]
pub(crate) struct InMemoryAnswerRelationRepository {
    relations: Mutex<Vec<Allowed<ReadableAnswerRelation, Read>>>,
}

impl InMemoryAnswerRelationRepository {
    pub(crate) fn set_authorized_relations(
        &self,
        relations: Vec<Allowed<ReadableAnswerRelation, Read>>,
    ) {
        *self.relations.lock().unwrap() = relations;
    }
}

#[async_trait]
impl AnswerRelationRepository for InMemoryAnswerRelationRepository {
    async fn list_for_answer(
        &self,
        source: &Allowed<AnswerEntry, Read>,
    ) -> Result<Vec<Allowed<ReadableAnswerRelation, Read>>, Error> {
        let actor = source.actor();
        let source = AnswerReference::new(*source.form_id(), *source.id());
        Ok(self
            .relations
            .lock()
            .unwrap()
            .iter()
            .filter(|relation| {
                relation.actor() == actor && relation.opposite_endpoint_for(source).is_ok()
            })
            .cloned()
            .collect())
    }

    async fn list_for_archived_answer(
        &self,
        source: &Allowed<ArchivedAnswerEntry, Read>,
    ) -> Result<Vec<Allowed<ReadableAnswerRelation, Read>>, Error> {
        let actor = source.actor();
        let source = AnswerReference::new(*source.form_id(), *source.id());
        Ok(self
            .relations
            .lock()
            .unwrap()
            .iter()
            .filter(|relation| {
                relation.actor() == actor && relation.opposite_endpoint_for(source).is_ok()
            })
            .cloned()
            .collect())
    }

    async fn add(
        &self,
        relation: AnswerRelation,
        source: &Allowed<AnswerEntry, Update>,
        target: &Allowed<AnswerEntry, Update>,
    ) -> Result<(), Error> {
        let authorized = relation.authorize_read_from_updates(source, target)?;
        let mut relations = self.relations.lock().unwrap();
        if !relations
            .iter()
            .any(|stored| stored.value().relation() == relation)
        {
            relations.push(authorized);
        }
        Ok(())
    }

    async fn remove(
        &self,
        relation: AnswerRelation,
        source: &Allowed<AnswerEntry, Update>,
        target: &Allowed<AnswerEntry, Update>,
    ) -> Result<(), Error> {
        let _ = relation.authorize_read_from_updates(source, target)?;
        self.relations
            .lock()
            .unwrap()
            .retain(|stored| stored.value().relation() != relation);
        Ok(())
    }

    async fn find_for_source_and_answer_id(
        &self,
        source: &Allowed<AnswerEntry, Update>,
        answer_id: AnswerId,
    ) -> Result<Option<Allowed<ReadableAnswerRelation, Read>>, Error> {
        let actor = source.actor();
        let source = AnswerReference::new(*source.form_id(), *source.id());
        Ok(self
            .relations
            .lock()
            .unwrap()
            .iter()
            .find(|relation| {
                relation.opposite_endpoint_for(source).is_ok_and(|target| {
                    relation.actor() == actor && target.answer_id() == answer_id
                })
            })
            .cloned())
    }
}

#[derive(Default)]
pub(crate) struct InMemoryArchivedFormRepository {
    forms: Mutex<Vec<ArchivedForm>>,
    answer_ids_by_form: Mutex<HashMap<FormId, Vec<AnswerId>>>,
}

impl InMemoryArchivedFormRepository {
    pub(crate) fn save_form_with_answers(&self, form: ArchivedForm, answer_ids: Vec<AnswerId>) {
        let form_id = *form.form().id();
        self.forms.lock().unwrap().push(form);
        self.answer_ids_by_form
            .lock()
            .unwrap()
            .insert(form_id, answer_ids);
    }
}

#[async_trait]
impl ArchivedFormRepository for InMemoryArchivedFormRepository {
    async fn list(
        &self,
        request: PageRequest<ArchivedFormPagePosition>,
        query: Option<String>,
    ) -> Result<Page<AuthorizationGuard<ArchivedForm, Read>, ArchivedFormPagePosition>, Error> {
        let mut forms = self
            .forms
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .filter(|form| match &query {
                Some(query) => {
                    form.form()
                        .title()
                        .to_owned()
                        .into_inner()
                        .into_inner()
                        .contains(query)
                        || form
                            .form()
                            .description()
                            .to_owned()
                            .into_inner()
                            .contains(query)
                }
                None => true,
            })
            .collect::<Vec<_>>();
        forms.sort_by(|left, right| {
            right
                .archived_at()
                .cmp(left.archived_at())
                .then_with(|| left.form().id().cmp(right.form().id()))
        });

        if let Some(position) = request.after_position() {
            forms.retain(|form| {
                *form.archived_at() < position.last_archived_at()
                    || (*form.archived_at() == position.last_archived_at()
                        && *form.form().id() > position.last_form_id())
            });
        }

        let page = Page::from_overfetched_items(forms, request.limit(), |form| {
            ArchivedFormPagePosition::new(*form.archived_at(), *form.form().id())
        });
        let (forms, next) = page.into_parts();

        Ok(Page::new(
            forms.into_iter().map(AuthorizationGuard::from).collect(),
            next,
        ))
    }

    async fn get(
        &self,
        id: FormId,
    ) -> Result<Option<AuthorizationGuard<ArchivedForm, Read>>, Error> {
        Ok(self
            .forms
            .lock()
            .unwrap()
            .iter()
            .find(|form| *form.form().id() == id)
            .cloned()
            .map(AuthorizationGuard::from))
    }

    async fn get_answer(
        &self,
        form: &Allowed<ArchivedForm, Read>,
        answer_id: AnswerId,
    ) -> Result<Option<Allowed<ArchivedAnswerEntry, Read>>, Error> {
        let exists = self
            .answer_ids_by_form
            .lock()
            .unwrap()
            .get(form.form().id())
            .is_some_and(|answer_ids| answer_ids.contains(&answer_id));
        if !exists {
            return Ok(None);
        }
        let answer = unsafe {
            ArchivedAnswerEntry::from_raw_parts(
                answer_id,
                *form.form().id(),
                AnswerPublication::PUBLIC,
            )
        };
        Ok(Some(form.read_archived_entry(answer)?))
    }

    async fn archive(
        &self,
        form: Allowed<ArchivedForm, Create>,
    ) -> Result<AuthorizationGuard<ArchivedForm, Read>, Error> {
        let form = form.into_inner();
        self.forms.lock().unwrap().push(form.clone());
        Ok(AuthorizationGuard::from(form))
    }

    async fn restore(&self, form: Allowed<ArchivedForm, Update>) -> Result<(), Error> {
        let restored_form_id = *form.form().id();
        self.forms
            .lock()
            .unwrap()
            .retain(|archived_form| *archived_form.form().id() != restored_form_id);
        self.answer_ids_by_form
            .lock()
            .unwrap()
            .remove(&restored_form_id);
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct InMemoryNotificationRepository {
    preferences: Mutex<Vec<NotificationPreference>>,
    notifications: Mutex<Vec<Notification>>,
}

#[async_trait]
impl NotificationRepository for InMemoryNotificationRepository {
    async fn create_notification(
        &self,
        notification: Allowed<Notification, Create>,
    ) -> Result<(), Error> {
        self.notifications
            .lock()
            .unwrap()
            .push(notification.into_inner());
        Ok(())
    }

    async fn fetch_notification(
        &self,
        id: NotificationId,
    ) -> Result<Option<AuthorizationGuard<Notification, Read>>, Error> {
        Ok(self
            .notifications
            .lock()
            .unwrap()
            .iter()
            .find(|notification| *notification.id() == id)
            .cloned()
            .map(AuthorizationGuard::from))
    }

    async fn fetch_notifications(
        &self,
        recipient_id: UserId,
        request: PageRequest<NotificationPagePosition>,
    ) -> Result<Page<AuthorizationGuard<Notification, Read>, NotificationPagePosition>, Error> {
        let mut notifications = self
            .notifications
            .lock()
            .unwrap()
            .iter()
            .filter(|notification| *notification.recipient_id() == recipient_id)
            .cloned()
            .collect::<Vec<_>>();
        notifications.sort_by(|left, right| right.id().cmp(left.id()));

        if let Some(position) = request.after_position() {
            notifications.retain(|notification| *notification.id() < position.id());
        }

        let page = Page::from_overfetched_items(notifications, request.limit(), |notification| {
            NotificationPagePosition::new(*notification.id())
        });
        let (notifications, next) = page.into_parts();

        Ok(Page::new(
            notifications
                .into_iter()
                .map(AuthorizationGuard::from)
                .collect(),
            next,
        ))
    }

    async fn update_notification(
        &self,
        notification: Allowed<Notification, Update>,
    ) -> Result<(), Error> {
        let notification = notification.into_inner();
        let notification_id = *notification.id();
        let mut notifications = self.notifications.lock().unwrap();
        if let Some(stored_notification) = notifications
            .iter_mut()
            .find(|stored| stored.id() == notification.id())
        {
            *stored_notification = notification;
            Ok(())
        } else {
            Err(not_found_error("Notification", notification_id))
        }
    }

    async fn update_read_at_for_actor(
        &self,
        actor: &Actor,
        read_at: DateTime<Utc>,
    ) -> Result<(), Error> {
        let recipient_id = match actor {
            Actor::AccountUser(actor) => *actor.id(),
            _ => return Err(errors::domain::DomainError::Forbidden.into()),
        };

        self.notifications
            .lock()
            .unwrap()
            .iter_mut()
            .filter(|notification| {
                *notification.recipient_id() == recipient_id && notification.read_at().is_none()
            })
            .for_each(|notification| {
                *notification = notification.clone().mark_as_read(read_at);
            });
        Ok(())
    }

    async fn create_notification_settings(
        &self,
        notification_settings: Allowed<NotificationPreference, Create>,
    ) -> Result<(), Error> {
        self.preferences
            .lock()
            .unwrap()
            .push(notification_settings.into_inner());
        Ok(())
    }

    async fn fetch_notification_settings(
        &self,
        recipient_id: Uuid,
    ) -> Result<Option<AuthorizationGuard<NotificationPreference, Read>>, Error> {
        Ok(self
            .preferences
            .lock()
            .unwrap()
            .iter()
            .find(|preference| preference.recipient_id().into_inner() == recipient_id)
            .cloned()
            .map(AuthorizationGuard::from))
    }

    async fn update_notification_settings(
        &self,
        notification_settings: Allowed<NotificationPreference, Update>,
    ) -> Result<(), Error> {
        let mut preferences = self.preferences.lock().unwrap();
        if let Some(stored_preference) = preferences.iter_mut().find(|stored| {
            stored.recipient_id().into_inner() == notification_settings.recipient_id().into_inner()
        }) {
            *stored_preference = notification_settings.into_inner();
            Ok(())
        } else {
            Err(not_found_error(
                "NotificationPreference",
                notification_settings.recipient_id(),
            ))
        }
    }
}

#[derive(Default)]
pub(crate) struct InMemoryUserRepository {
    users: Mutex<Vec<AccountUser>>,
    groups: Mutex<Vec<UserGroup>>,
    sessions: Mutex<Vec<(String, AccountUser)>>,
}

impl InMemoryUserRepository {
    pub(crate) fn save_user(&self, user: AccountUser) {
        self.users.lock().unwrap().push(user);
    }
}

#[async_trait]
impl UserRepository for InMemoryUserRepository {
    async fn find_by(
        &self,
        uuid: Uuid,
    ) -> Result<Option<AuthorizationGuard<AccountUser, Read>>, Error> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .iter()
            .find(|user| user.id().into_inner() == uuid)
            .cloned()
            .map(AuthorizationGuard::from))
    }

    async fn find_by_ids(
        &self,
        uuids: Vec<Uuid>,
    ) -> Result<Vec<AuthorizationGuard<AccountUser, Read>>, Error> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .iter()
            .filter(|user| uuids.contains(&user.id().into_inner()))
            .cloned()
            .map(AuthorizationGuard::from)
            .collect())
    }

    async fn upsert_user(&self, user: Allowed<AccountUser, Create>) -> Result<(), Error> {
        let user = user.into_inner();
        let mut users = self.users.lock().unwrap();
        if let Some(stored_user) = users.iter_mut().find(|stored| stored.id() == user.id()) {
            *stored_user = user;
        } else {
            users.push(user);
        }
        Ok(())
    }

    async fn patch_user_role(&self, user: Allowed<AccountUser, Update>) -> Result<(), Error> {
        let user = user.into_inner();
        let mut users = self.users.lock().unwrap();
        if let Some(stored_user) = users.iter_mut().find(|stored| stored.id() == user.id()) {
            *stored_user = user;
            Ok(())
        } else {
            Err(not_found_error("AccountUser", user.id()))
        }
    }

    async fn create_user_group(&self, group: Allowed<UserGroup, Create>) -> Result<(), Error> {
        self.groups.lock().unwrap().push(group.into_inner());
        Ok(())
    }

    async fn update_user_group(&self, group: Allowed<UserGroup, Update>) -> Result<(), Error> {
        let group = group.into_inner();
        let mut groups = self.groups.lock().unwrap();
        if let Some(stored_group) = groups.iter_mut().find(|stored| stored.id() == group.id()) {
            *stored_group = group;
            Ok(())
        } else {
            Err(not_found_error("UserGroup", group.id()))
        }
    }

    async fn delete_user_group(&self, group: Allowed<UserGroup, Delete>) -> Result<(), Error> {
        self.groups
            .lock()
            .unwrap()
            .retain(|stored| stored.id() != group.id());
        Ok(())
    }

    async fn find_user_group(
        &self,
        group_id: UserGroupId,
    ) -> Result<Option<AuthorizationGuard<UserGroup, Read>>, Error> {
        Ok(self
            .groups
            .lock()
            .unwrap()
            .iter()
            .find(|group| *group.id() == group_id)
            .cloned()
            .map(AuthorizationGuard::from))
    }

    async fn fetch_user_groups(&self) -> Result<Vec<AuthorizationGuard<UserGroup, Read>>, Error> {
        Ok(self
            .groups
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .map(AuthorizationGuard::from)
            .collect())
    }

    async fn fetch_users_by_group(
        &self,
        group: Allowed<UserGroup, Read>,
    ) -> Result<Vec<AuthorizationGuard<AccountUser, Read>>, Error> {
        let group_id = *group.id();
        let mut users = self
            .users
            .lock()
            .unwrap()
            .iter()
            .filter(|user| user.groups().iter().any(|group| *group.id() == group_id))
            .cloned()
            .collect::<Vec<_>>();
        users.sort_by_key(|user| user.id().into_inner());

        Ok(users.into_iter().map(AuthorizationGuard::from).collect())
    }

    async fn add_user_to_group(
        &self,
        group: Allowed<UserGroup, Update>,
        user: Allowed<AccountUser, Update>,
    ) -> Result<(), Error> {
        let mut users = self.users.lock().unwrap();
        let Some(stored_user) = users.iter_mut().find(|stored| stored.id() == user.id()) else {
            return Err(not_found_error("AccountUser", user.id()));
        };
        if !stored_user
            .groups()
            .iter()
            .any(|stored| stored.id() == group.id())
        {
            let mut groups = stored_user.groups().to_vec();
            groups.push(group.into_inner());
            *stored_user = AccountUser::with_groups(
                stored_user.name().to_owned(),
                *stored_user.id(),
                stored_user.role().to_owned(),
                groups,
            );
        }
        Ok(())
    }

    async fn remove_user_from_group(
        &self,
        group: Allowed<UserGroup, Update>,
        user: Allowed<AccountUser, Update>,
    ) -> Result<(), Error> {
        let mut users = self.users.lock().unwrap();
        let Some(stored_user) = users.iter_mut().find(|stored| stored.id() == user.id()) else {
            return Err(not_found_error("AccountUser", user.id()));
        };
        let groups = stored_user
            .groups()
            .iter()
            .filter(|stored| stored.id() != group.id())
            .cloned()
            .collect();
        *stored_user = AccountUser::with_groups(
            stored_user.name().to_owned(),
            *stored_user.id(),
            stored_user.role().to_owned(),
            groups,
        );
        Ok(())
    }

    async fn fetch_user_by_xbox_token(&self, _token: String) -> Result<Option<AccountUser>, Error> {
        Ok(None)
    }

    async fn fetch_all_users(&self) -> Result<Vec<AuthorizationGuard<AccountUser, Read>>, Error> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .map(AuthorizationGuard::from)
            .collect())
    }

    async fn fetch_users_page(
        &self,
        request: PageRequest<UserPagePosition>,
    ) -> Result<Page<AuthorizationGuard<AccountUser, Read>, UserPagePosition>, Error> {
        let mut users = self.users.lock().unwrap().clone();
        users.sort_by_key(|user| user.id().into_inner());

        if let Some(position) = request.after_position() {
            users.retain(|user| *user.id() > position.last_user_id());
        }

        let page = Page::from_overfetched_items(users, request.limit(), |user| {
            UserPagePosition::new(*user.id())
        });
        let (users, next) = page.into_parts();

        Ok(Page::new(
            users.into_iter().map(AuthorizationGuard::from).collect(),
            next,
        ))
    }

    async fn start_user_session(
        &self,
        xbox_token: String,
        user: &AccountUser,
        _expires: u32,
    ) -> Result<String, Error> {
        self.sessions
            .lock()
            .unwrap()
            .push((xbox_token.clone(), user.clone()));
        Ok(xbox_token)
    }

    async fn fetch_user_by_session_id(
        &self,
        session_id: String,
    ) -> Result<Option<AccountUser>, Error> {
        Ok(self
            .sessions
            .lock()
            .unwrap()
            .iter()
            .find(|(stored_session_id, _)| stored_session_id == &session_id)
            .map(|(_, user)| user.clone()))
    }

    async fn end_user_session(&self, session_id: String) -> Result<(), Error> {
        self.sessions
            .lock()
            .unwrap()
            .retain(|(stored_session_id, _)| stored_session_id != &session_id);
        Ok(())
    }

    async fn link_discord_user(
        &self,
        _link: Allowed<DiscordAccountLink, Update>,
    ) -> Result<(), Error> {
        Ok(())
    }

    async fn unlink_discord_user(
        &self,
        _link: Allowed<DiscordAccountLink, Delete>,
    ) -> Result<(), Error> {
        Ok(())
    }

    async fn fetch_discord_user(
        &self,
        _user: &Allowed<AccountUser, Read>,
    ) -> Result<Option<DiscordUser>, Error> {
        Ok(None)
    }

    async fn fetch_discord_user_by_token(
        &self,
        _token: String,
    ) -> Result<Option<DiscordUser>, Error> {
        Ok(None)
    }

    async fn size(&self) -> Result<u32, Error> {
        Ok(self.users.lock().unwrap().len() as u32)
    }
}

#[derive(Default)]
pub(crate) struct InMemoryFormSubmissionRestrictionRepository {
    restrictions: Mutex<Vec<FormSubmissionRestriction>>,
}

impl InMemoryFormSubmissionRestrictionRepository {
    pub(crate) fn save_form_submission_restriction(&self, restriction: FormSubmissionRestriction) {
        self.restrictions.lock().unwrap().push(restriction);
    }
}

#[async_trait]
impl FormSubmissionRestrictionRepository for InMemoryFormSubmissionRestrictionRepository {
    async fn fetch_active_by_submitter_id(
        &self,
        submitter_id: Uuid,
    ) -> Result<Option<AuthorizationGuard<FormSubmissionRestriction, Read>>, Error> {
        Ok(self
            .restrictions
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|restriction| {
                restriction.submitter_id().into_inner() == submitter_id
                    && restriction.is_active_at(chrono::Utc::now())
            })
            .cloned()
            .map(Into::into))
    }

    async fn list_by_submitter_id(
        &self,
        submitter_id: Uuid,
    ) -> Result<AuthorizationGuard<FormSubmissionRestrictionHistory, Read>, Error> {
        Ok(FormSubmissionRestrictionHistory::new(
            submitter_id.into(),
            self.restrictions
                .lock()
                .unwrap()
                .iter()
                .rev()
                .filter(|restriction| restriction.submitter_id().into_inner() == submitter_id)
                .cloned()
                .collect(),
        )?
        .into())
    }

    async fn restrict(
        &self,
        restriction: Allowed<FormSubmissionRestriction, Create>,
    ) -> Result<(), Error> {
        let restriction = restriction.into_inner();
        let mut restrictions = self.restrictions.lock().unwrap();
        restrictions
            .iter_mut()
            .filter(|stored| {
                stored.submitter_id() == restriction.submitter_id()
                    && stored.is_active_at(chrono::Utc::now())
            })
            .for_each(|stored| {
                *stored = lifted_form_submission_restriction(
                    stored,
                    chrono::Utc::now(),
                    *restriction.restricted_by(),
                );
            });
        restrictions.push(restriction);
        Ok(())
    }

    async fn lift(
        &self,
        restriction: Allowed<FormSubmissionRestriction, Delete>,
    ) -> Result<(), Error> {
        let lifted_by = match restriction.actor() {
            Actor::AccountUser(user) => *user.id(),
            _ => return Ok(()),
        };

        self.restrictions
            .lock()
            .unwrap()
            .iter_mut()
            .filter(|stored| {
                stored.submitter_id() == restriction.submitter_id()
                    && stored.is_active_at(chrono::Utc::now())
            })
            .for_each(|stored| {
                *stored = lifted_form_submission_restriction(stored, chrono::Utc::now(), lifted_by);
            });
        Ok(())
    }
}

fn lifted_form_submission_restriction(
    restriction: &FormSubmissionRestriction,
    lifted_at: chrono::DateTime<chrono::Utc>,
    lifted_by: domain::account::models::UserId,
) -> FormSubmissionRestriction {
    unsafe {
        FormSubmissionRestriction::from_raw_parts(
            FormSubmissionRestrictionId::from(restriction.id().into_inner()),
            *restriction.submitter_id(),
            restriction.reason().clone(),
            *restriction.restricted_by(),
            *restriction.restricted_at(),
            *restriction.expires_at(),
            Some(lifted_at),
            Some(lifted_by),
        )
    }
}
