use async_trait::async_trait;
use domain::{
    form::{
        answer::{AnswerEntry, AnswerId},
        models::{ActiveForm, ArchivedForm, FormId, FormLabel, FormLabelId},
    },
    notification::models::NotificationPreference,
    repository::{
        form::{
            active_form_repository::ActiveFormRepository,
            answer_entry_repository::AnswerEntryRepository,
            archived_form_repository::ArchivedFormRepository,
            form_label_repository::FormLabelRepository,
        },
        notification_repository::NotificationRepository,
        user_repository::UserRepository,
    },
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Delete, Read, Update},
    user::models::{ActiveUser, AnswerSubmissionRestriction, DiscordAccountLink, DiscordUser},
};
use errors::Error;
use std::sync::Mutex;
use uuid::Uuid;

use crate::forms::form::FormUseCase;

fn paginate<T>(
    items: impl IntoIterator<Item = T>,
    offset: Option<u32>,
    limit: Option<u32>,
) -> Vec<T> {
    let items = items.into_iter().skip(offset.unwrap_or(0) as usize);
    match limit {
        Some(limit) => items.take(limit as usize).collect(),
        None => items.collect(),
    }
}

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
    pub(crate) user_repository: InMemoryUserRepository,
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
}

#[async_trait]
impl ActiveFormRepository for InMemoryActiveFormRepository {
    async fn create(
        &self,
        _actor: &ActiveUser,
        form: Allowed<ActiveForm, Create>,
    ) -> Result<(), Error> {
        self.save_form(form.into_inner());
        Ok(())
    }

    async fn list(
        &self,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Vec<AuthorizationGuard<ActiveForm, Read>>, Error> {
        let forms = self
            .forms
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .collect::<Vec<_>>();

        Ok(paginate(forms, offset, limit)
            .into_iter()
            .map(AuthorizationGuard::from)
            .collect())
    }

    async fn get(&self, id: FormId) -> Result<Option<AuthorizationGuard<ActiveForm, Read>>, Error> {
        Ok(self.find_form(id).map(AuthorizationGuard::from))
    }

    async fn update_form(
        &self,
        _actor: &ActiveUser,
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

    async fn list_by_form(
        &self,
        form: &Allowed<ActiveForm, Read>,
    ) -> Result<Vec<Allowed<AnswerEntry, Read>>, Error> {
        Ok(form.readable_entries(
            self.answers
                .lock()
                .unwrap()
                .iter()
                .filter(|answer| answer.form_id() == form.id())
                .cloned()
                .collect(),
        ))
    }

    async fn list_all(
        &self,
        forms: &[Allowed<ActiveForm, Read>],
    ) -> Result<Vec<Allowed<AnswerEntry, Read>>, Error> {
        let answers = self.answers.lock().unwrap();
        Ok(forms
            .iter()
            .flat_map(|form| {
                form.readable_entries(
                    answers
                        .iter()
                        .filter(|answer| answer.form_id() == form.id())
                        .cloned()
                        .collect(),
                )
            })
            .collect())
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
    ) -> Result<(), Error> {
        let mut answers = self.answers.lock().unwrap();
        if let Some(stored_answer) = answers
            .iter_mut()
            .find(|stored| *stored.id() == *answer_entry.id())
        {
            *stored_answer = answer_entry.value().clone();
            Ok(())
        } else {
            Err(not_found_error("AnswerEntry", answer_entry.id()))
        }
    }

    async fn size(&self) -> Result<u32, Error> {
        Ok(self.answers.lock().unwrap().len() as u32)
    }
}

#[derive(Default)]
pub(crate) struct InMemoryArchivedFormRepository {
    forms: Mutex<Vec<ArchivedForm>>,
}

#[async_trait]
impl ArchivedFormRepository for InMemoryArchivedFormRepository {
    async fn list(
        &self,
        offset: Option<u32>,
        limit: Option<u32>,
        query: Option<String>,
    ) -> Result<Vec<AuthorizationGuard<ArchivedForm, Read>>, Error> {
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
        forms.sort_by(|left, right| right.archived_at().cmp(left.archived_at()));

        Ok(paginate(forms, offset, limit)
            .into_iter()
            .map(AuthorizationGuard::from)
            .collect())
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
        Ok(())
    }
}

#[derive(Default)]
pub(crate) struct InMemoryNotificationRepository {
    preferences: Mutex<Vec<NotificationPreference>>,
}

#[async_trait]
impl NotificationRepository for InMemoryNotificationRepository {
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
    users: Mutex<Vec<ActiveUser>>,
    sessions: Mutex<Vec<(String, ActiveUser)>>,
    answer_submission_restrictions: Mutex<Vec<AnswerSubmissionRestriction>>,
}

impl InMemoryUserRepository {
    pub(crate) fn save_answer_submission_restriction(
        &self,
        restriction: AnswerSubmissionRestriction,
    ) {
        self.answer_submission_restrictions
            .lock()
            .unwrap()
            .push(restriction);
    }
}

#[async_trait]
impl UserRepository for InMemoryUserRepository {
    async fn find_by(
        &self,
        uuid: Uuid,
    ) -> Result<Option<AuthorizationGuard<ActiveUser, Read>>, Error> {
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
    ) -> Result<Vec<AuthorizationGuard<ActiveUser, Read>>, Error> {
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

    async fn upsert_user(&self, user: Allowed<ActiveUser, Create>) -> Result<(), Error> {
        let user = user.into_inner();
        let mut users = self.users.lock().unwrap();
        if let Some(stored_user) = users.iter_mut().find(|stored| stored.id() == user.id()) {
            *stored_user = user;
        } else {
            users.push(user);
        }
        Ok(())
    }

    async fn patch_user_role(&self, user: Allowed<ActiveUser, Update>) -> Result<(), Error> {
        let user = user.into_inner();
        let mut users = self.users.lock().unwrap();
        if let Some(stored_user) = users.iter_mut().find(|stored| stored.id() == user.id()) {
            *stored_user = user;
            Ok(())
        } else {
            Err(not_found_error("ActiveUser", user.id()))
        }
    }

    async fn fetch_active_answer_submission_restriction(
        &self,
        user_id: Uuid,
    ) -> Result<Option<AnswerSubmissionRestriction>, Error> {
        Ok(self
            .answer_submission_restrictions
            .lock()
            .unwrap()
            .iter()
            .find(|restriction| {
                restriction.user_id().into_inner() == user_id
                    && restriction.is_active_at(chrono::Utc::now())
            })
            .cloned())
    }

    async fn restrict_answer_submission(
        &self,
        restriction: Allowed<AnswerSubmissionRestriction, Create>,
    ) -> Result<(), Error> {
        self.save_answer_submission_restriction(restriction.into_inner());
        Ok(())
    }

    async fn lift_answer_submission_restriction(
        &self,
        user_id: Uuid,
        _actor: &ActiveUser,
    ) -> Result<(), Error> {
        self.answer_submission_restrictions
            .lock()
            .unwrap()
            .retain(|restriction| restriction.user_id().into_inner() != user_id);
        Ok(())
    }

    async fn fetch_user_by_xbox_token(&self, _token: String) -> Result<Option<ActiveUser>, Error> {
        Ok(None)
    }

    async fn fetch_all_users(&self) -> Result<Vec<AuthorizationGuard<ActiveUser, Read>>, Error> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .map(AuthorizationGuard::from)
            .collect())
    }

    async fn start_user_session(
        &self,
        xbox_token: String,
        user: &ActiveUser,
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
    ) -> Result<Option<ActiveUser>, Error> {
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
        _user: &Allowed<ActiveUser, Read>,
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
