use async_trait::async_trait;
use domain::{
    account::models::{
        AccountUser, DiscordAccountLink, DiscordUser, UserGroup, UserGroupId, UserPagePosition,
    },
    auth::Actor,
    form::{
        answer::{
            AnswerEntry, AnswerId, AnswerPagePosition, AnswerSubmitterRestriction,
            AnswerSubmitterRestrictionHistory, AnswerSubmitterRestrictionId,
        },
        models::{
            ActiveForm, ArchivedForm, ArchivedFormPagePosition, FormId, FormLabel, FormLabelId,
            FormPagePosition,
        },
    },
    notification::models::NotificationPreference,
    pagination::{Page, PageRequest},
    repository::{
        answer_submitter_restriction_repository::AnswerSubmitterRestrictionRepository,
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
};
use errors::Error;
use std::sync::Mutex;
use uuid::Uuid;

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
    pub(crate) user_repository: InMemoryUserRepository,
    pub(crate) answer_submitter_restriction_repository:
        InMemoryAnswerSubmitterRestrictionRepository,
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
        request: PageRequest<AnswerPagePosition>,
    ) -> Result<Page<Allowed<AnswerEntry, Read>, AnswerPagePosition>, Error> {
        let mut answers = self
            .answers
            .lock()
            .unwrap()
            .iter()
            .filter(|answer| answer.form_id() == form.id())
            .cloned()
            .collect::<Vec<_>>();
        answers.sort_by_key(|answer| answer.id().into_inner());

        if let Some(position) = request.after_position() {
            answers.retain(|answer| *answer.id() > position.last_answer_id());
        }

        let page = Page::from_overfetched_items(answers, request.limit(), |answer| {
            AnswerPagePosition::new(*answer.id())
        });
        let (answers, next) = page.into_parts();

        Ok(Page::new(form.readable_entries(answers), next))
    }

    async fn list_all(
        &self,
        forms: &[Allowed<ActiveForm, Read>],
        request: PageRequest<AnswerPagePosition>,
    ) -> Result<Page<Allowed<AnswerEntry, Read>, AnswerPagePosition>, Error> {
        let mut answers = self.answers.lock().unwrap().clone();
        answers.sort_by_key(|answer| answer.id().into_inner());

        if let Some(position) = request.after_position() {
            answers.retain(|answer| *answer.id() > position.last_answer_id());
        }

        let page = Page::from_overfetched_items(answers, request.limit(), |answer| {
            AnswerPagePosition::new(*answer.id())
        });
        let (answers, next) = page.into_parts();
        let readable_answers = forms
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
            .collect::<Vec<_>>();

        Ok(Page::new(readable_answers, next))
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
pub(crate) struct InMemoryAnswerSubmitterRestrictionRepository {
    restrictions: Mutex<Vec<AnswerSubmitterRestriction>>,
}

impl InMemoryAnswerSubmitterRestrictionRepository {
    pub(crate) fn save_answer_submitter_restriction(
        &self,
        restriction: AnswerSubmitterRestriction,
    ) {
        self.restrictions.lock().unwrap().push(restriction);
    }
}

#[async_trait]
impl AnswerSubmitterRestrictionRepository for InMemoryAnswerSubmitterRestrictionRepository {
    async fn fetch_active_by_submitter_id(
        &self,
        submitter_id: Uuid,
    ) -> Result<Option<AuthorizationGuard<AnswerSubmitterRestriction, Read>>, Error> {
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
    ) -> Result<AuthorizationGuard<AnswerSubmitterRestrictionHistory, Read>, Error> {
        Ok(AnswerSubmitterRestrictionHistory::new(
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
        restriction: Allowed<AnswerSubmitterRestriction, Create>,
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
                *stored = lifted_answer_submitter_restriction(
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
        restriction: Allowed<AnswerSubmitterRestriction, Delete>,
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
                *stored =
                    lifted_answer_submitter_restriction(stored, chrono::Utc::now(), lifted_by);
            });
        Ok(())
    }
}

fn lifted_answer_submitter_restriction(
    restriction: &AnswerSubmitterRestriction,
    lifted_at: chrono::DateTime<chrono::Utc>,
    lifted_by: domain::account::models::UserId,
) -> AnswerSubmitterRestriction {
    unsafe {
        AnswerSubmitterRestriction::from_raw_parts(
            AnswerSubmitterRestrictionId::from(restriction.id().into_inner()),
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
