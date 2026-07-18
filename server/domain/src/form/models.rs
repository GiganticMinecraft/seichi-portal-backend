use chrono::{DateTime, Utc};
#[cfg(test)]
use common::test_utils::arbitrary_date_time;
use derive_getters::Getters;
use deriving_via::DerivingVia;
use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;
#[cfg(test)]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};
use types::non_empty_string::NonEmptyString;

pub use crate::form::{
    answer::{AnswerAcceptancePeriod, AnswerSettings, AnswerVisibility, DefaultAnswerTitle},
    label::{FormLabel, FormLabelAssignment, FormLabelId, FormLabelName},
    question::{Question, QuestionSet},
    settings::{AllowedUserGroups, DiscordWebhookUrl, FormSettings, Visibility},
};

use crate::{
    account::models::UserId,
    auth::Actor,
    form::answer::TemporaryAnswerAuthor,
    form::{
        answer::{AnswerAuthor, AnswerEntry, AnswerSubmitter, AnswerTitle, PostedAnswerContents},
        is_administrator,
    },
    types::authorization_guard::{
        Allowed, AuthorizationGuardDefinitions, AuthorizationRole, Create, Read, SelfGuarded,
        Update,
    },
};

pub type FormId = types::Id<ActiveForm>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormPagePosition {
    last_form_id: FormId,
}

impl FormPagePosition {
    pub fn new(last_form_id: FormId) -> Self {
        Self { last_form_id }
    }

    pub fn last_form_id(self) -> FormId {
        self.last_form_id
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArchivedFormPagePosition {
    last_archived_at: DateTime<Utc>,
    last_form_id: FormId,
}

impl ArchivedFormPagePosition {
    pub fn new(last_archived_at: DateTime<Utc>, last_form_id: FormId) -> Self {
        Self {
            last_archived_at,
            last_form_id,
        }
    }

    pub fn last_archived_at(self) -> DateTime<Utc> {
        self.last_archived_at
    }

    pub fn last_form_id(self) -> FormId {
        self.last_form_id
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Clone, DerivingVia, Debug, PartialOrd, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize, Deserialize)]
pub struct FormTitle(NonEmptyString);

impl FormTitle {
    pub fn new(title: NonEmptyString) -> Self {
        Self(title)
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Clone, DerivingVia, Default, Debug, PartialOrd, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize(via: String), Deserialize(via: String
))]
pub struct FormDescription(String);

impl FormDescription {
    pub fn new(description: String) -> Self {
        Self(description)
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(UnsafeFromRawParts, Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
pub struct FormMeta {
    #[cfg_attr(test, proptest(strategy = "arbitrary_date_time()"))]
    #[serde(default = "chrono::Utc::now")]
    pub created_at: DateTime<Utc>,
    #[cfg_attr(test, proptest(strategy = "arbitrary_date_time()"))]
    #[serde(default = "chrono::Utc::now")]
    pub updated_at: DateTime<Utc>,
}

impl FormMeta {
    pub fn new() -> Self {
        Self {
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(UnsafeFromRawParts, Serialize, Deserialize, Getters, Clone, Debug, PartialEq)]
pub struct ActiveForm {
    #[serde(default)]
    id: FormId,
    title: FormTitle,
    #[serde(default)]
    description: FormDescription,
    #[serde(default)]
    metadata: FormMeta,
    #[serde(default)]
    settings: FormSettings,
    #[serde(default)]
    answer_settings: AnswerSettings,
    questions: QuestionSet,
    #[serde(default)]
    label_ids: FormLabelAssignment,
}

impl ActiveForm {
    pub fn new(title: FormTitle, description: FormDescription, questions: QuestionSet) -> Self {
        Self {
            id: FormId::new(),
            title,
            description,
            metadata: FormMeta::new(),
            settings: FormSettings::new(),
            answer_settings: AnswerSettings::default(),
            questions,
            label_ids: FormLabelAssignment::empty(),
        }
    }

    pub fn change_title(self, title: FormTitle) -> Self {
        Self { title, ..self }
    }

    pub fn change_description(self, description: FormDescription) -> Self {
        Self {
            description,
            ..self
        }
    }

    pub fn change_settings(self, settings: FormSettings) -> Self {
        Self { settings, ..self }
    }

    pub fn change_answer_settings(self, answer_settings: AnswerSettings) -> Self {
        Self {
            answer_settings,
            ..self
        }
    }

    pub fn change_questions(self, questions: QuestionSet) -> Self {
        Self { questions, ..self }
    }

    pub fn replace_label_ids(self, label_ids: FormLabelAssignment) -> Self {
        Self { label_ids, ..self }
    }

    fn try_accept_answer_from_submitter(
        &self,
        submitter: AnswerSubmitter,
        title: AnswerTitle,
        posted_answers: PostedAnswerContents,
    ) -> Result<AnswerEntry, DomainError> {
        let user = submitter.into_user();
        let author = AnswerAuthor::AuthenticatedUser(*user.id());
        let actor = Actor::from(user);

        if !self.answer_settings.can_accept_answer(&author, &actor) {
            return Err(DomainError::Forbidden);
        }
        Ok(AnswerEntry::new(*self.id(), author, title, posted_answers))
    }

    fn try_accept_temporary_answer(
        &self,
        temporary_user: TemporaryAnswerAuthor,
        title: AnswerTitle,
        posted_answers: PostedAnswerContents,
    ) -> Result<AnswerEntry, DomainError> {
        let actor = Actor::from(temporary_user.clone());
        let author = AnswerAuthor::Temporary(temporary_user);

        if !self.answer_settings.can_accept_answer(&author, &actor) {
            return Err(DomainError::Forbidden);
        }
        Ok(AnswerEntry::new(*self.id(), author, title, posted_answers))
    }

    pub fn archive(self, archived_at: DateTime<Utc>, archived_by: UserId) -> ArchivedForm {
        ArchivedForm::new(self, archived_at, archived_by)
    }
}

impl Allowed<ActiveForm, Read> {
    pub fn try_accept_answer(
        &self,
        submitter: AnswerSubmitter,
        title: AnswerTitle,
        posted_answers: PostedAnswerContents,
    ) -> Result<Allowed<AnswerEntry, Create>, DomainError> {
        let entry =
            self.value()
                .try_accept_answer_from_submitter(submitter, title, posted_answers)?;
        self.authorize_create(entry)
    }

    pub fn try_accept_temporary_answer(
        &self,
        temporary_user: TemporaryAnswerAuthor,
        title: AnswerTitle,
        posted_answers: PostedAnswerContents,
    ) -> Result<Allowed<AnswerEntry, Create>, DomainError> {
        let entry =
            self.value()
                .try_accept_temporary_answer(temporary_user, title, posted_answers)?;
        self.authorize_create(entry)
    }

    /// `entries` のうち `actor` が閲覧可能な [`AnswerEntry`] だけを認可済みで返します。
    /// 所属 (`form_id` 一致) と公開範囲はいずれも
    /// [`GuardedBy`](crate::types::authorization_guard::GuardedBy) の判定で担保される。
    pub fn readable_entries(&self, entries: Vec<AnswerEntry>) -> Vec<Allowed<AnswerEntry, Read>> {
        entries
            .into_iter()
            .filter_map(|entry| self.authorize_read(entry).ok())
            .collect()
    }

    /// `entry` を、所属 (`form_id` 一致) と公開範囲を検証したうえで認可済みで返します。
    pub fn read_entry(
        &self,
        entry: AnswerEntry,
    ) -> Result<Allowed<AnswerEntry, Read>, DomainError> {
        self.authorize_read(entry)
    }
}

impl Allowed<ActiveForm, Update> {
    /// `entry` のタイトルだけを変更し、更新認可済みで返します。タイトル以外が変わらないことは
    /// [`AnswerEntry::with_title`] による構築で保証され、所属と更新権限は [`ActiveForm`] の
    /// ガード経由で保証される。
    pub fn change_entry_title(
        &self,
        entry: AnswerEntry,
        title: AnswerTitle,
    ) -> Result<Allowed<AnswerEntry, Update>, DomainError> {
        self.authorize_update(entry.with_title(title))
    }
}

#[derive(UnsafeFromRawParts, Serialize, Deserialize, Getters, Clone, Debug, PartialEq)]
pub struct ArchivedForm {
    form: ActiveForm,
    archived_at: DateTime<Utc>,
    archived_by: UserId,
}

impl ArchivedForm {
    pub fn new(form: ActiveForm, archived_at: DateTime<Utc>, archived_by: UserId) -> Self {
        Self {
            form,
            archived_at,
            archived_by,
        }
    }

    pub fn unarchive(self) -> ActiveForm {
        self.form
    }
}

impl AuthorizationRole for ArchivedForm {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for ArchivedForm {
    fn can_create(&self, actor: &Actor) -> bool {
        is_administrator(actor)
    }

    fn can_read(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::System) || is_administrator(actor)
    }

    fn can_update(&self, actor: &Actor) -> bool {
        is_administrator(actor)
    }

    fn can_delete(&self, actor: &Actor) -> bool {
        is_administrator(actor)
    }
}

impl AuthorizationRole for ActiveForm {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for ActiveForm {
    /// [`ActiveForm`] の作成権限があるかどうかを判定します。
    ///
    /// 作成権限は [`Administrator`](crate::account::models::Role::Administrator) のみに与えられます。
    fn can_create(&self, actor: &Actor) -> bool {
        is_administrator(actor)
    }

    /// [`ActiveForm`] の読み取り権限があるかどうかを判定します。
    ///
    /// 読み取り権限は以下のいずれかを満たす場合に与えられます。
    /// - [`Actor::System`] である場合
    /// - [`FormSettings`] の [`Visibility`] が [`Visibility::PUBLIC`] である場合
    /// - [`Administrator`](crate::account::models::Role::Administrator) である場合
    fn can_read(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::System)
            || (self.settings.visibility() == &Visibility::PUBLIC
                && self.settings.allowed_user_groups().allows(actor))
            || is_administrator(actor)
    }

    /// [`ActiveForm`] の更新権限があるかどうかを判定します。
    ///
    /// 更新権限は [`Administrator`](crate::account::models::Role::Administrator) のみに与えられます。
    fn can_update(&self, actor: &Actor) -> bool {
        is_administrator(actor)
    }

    /// [`ActiveForm`] の削除権限は常に与えられません。
    ///
    /// 削除は [`ArchivedForm`] へのアーカイブ操作を経由してください。
    fn can_delete(&self, _actor: &Actor) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        account::models::{AccountUser, Role, UserGroup, UserGroupId, UserGroupName},
        form::answer::TemporaryAnswerAuthor,
        form::{
            answer::{AnswerSubmitter, FormAnswerContent, FormAnswerContentId},
            question::{Question, QuestionId, QuestionType},
        },
        types::authorization_guard::{AuthorizationGuard, Read},
    };
    use chrono::Duration;
    use types::non_empty_vec::NonEmptyVec;
    use uuid::Uuid;

    fn sample_question_set() -> QuestionSet {
        QuestionSet::try_new(
            NonEmptyVec::try_new(vec![unsafe {
                Question::from_raw_parts(
                    QuestionId::from(Uuid::new_v4()),
                    "body".to_string().try_into().unwrap(),
                    0,
                    "Body".to_string().try_into().unwrap(),
                    None,
                    QuestionType::Text,
                    None,
                    true,
                )
                .unwrap()
            }])
            .unwrap(),
        )
        .unwrap()
    }

    fn sample_form() -> ActiveForm {
        ActiveForm::new(
            FormTitle::new("Form".to_string().try_into().unwrap()),
            FormDescription::new("description".to_string()),
            sample_question_set(),
        )
    }

    fn active_user(role: Role) -> AccountUser {
        AccountUser::new("user".to_string(), UserId::from(Uuid::new_v4()), role)
    }

    fn user_group(seed: u128, name: &str) -> UserGroup {
        unsafe {
            UserGroup::from_raw_parts(
                UserGroupId::from(Uuid::from_u128(seed)),
                UserGroupName::new(name.to_string().try_into().unwrap()),
            )
        }
    }

    fn active_user_with_groups(role: Role, groups: Vec<UserGroup>) -> AccountUser {
        AccountUser::with_groups(
            "user".to_string(),
            UserId::from(Uuid::new_v4()),
            role,
            groups,
        )
    }

    fn sample_posted_answers(form: &ActiveForm) -> PostedAnswerContents {
        PostedAnswerContents::try_new(
            form.questions().as_slice(),
            vec![FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: (*form.questions().as_slice()[0].id()).into(),
                answer: "answer".to_string(),
            }],
        )
        .unwrap()
    }

    fn public_form_read_by(form: ActiveForm, actor: Actor) -> Allowed<ActiveForm, Read> {
        AuthorizationGuard::<_, Read>::from(form)
            .try_read(actor)
            .unwrap()
    }

    #[test]
    fn try_accept_answer_respects_temporary_answer_settings() {
        let temporary_user = TemporaryAnswerAuthor::new("guest".to_string(), "contact".to_string());
        let actor = Actor::from(temporary_user.clone());

        let form = sample_form();
        let denied_result = public_form_read_by(form.clone(), actor.clone())
            .try_accept_temporary_answer(
                temporary_user.clone(),
                AnswerTitle::new(None),
                sample_posted_answers(&form),
            );

        assert!(matches!(denied_result, Err(DomainError::Forbidden)));

        let form = form
            .change_answer_settings(AnswerSettings::default().change_allow_temporary_answers(true));
        let accepted_result = public_form_read_by(form.clone(), actor).try_accept_temporary_answer(
            temporary_user,
            AnswerTitle::new(None),
            sample_posted_answers(&form),
        );

        assert!(accepted_result.is_ok());
    }

    #[test]
    fn form_readability_does_not_imply_private_answer_readability() {
        let answer_author = active_user(Role::StandardUser);
        let other_user = active_user(Role::StandardUser);
        let form = sample_form();
        let entry = AnswerEntry::new(
            *form.id(),
            AnswerAuthor::AuthenticatedUser(*answer_author.id()),
            AnswerTitle::new(None),
            sample_posted_answers(&form),
        );

        let private_answer_read_by_author =
            public_form_read_by(form.clone(), Actor::from(answer_author)).read_entry(entry.clone());
        let private_answer_read_by_other_user =
            public_form_read_by(form.clone(), Actor::from(other_user.clone()))
                .read_entry(entry.clone());

        assert!(private_answer_read_by_author.is_ok());
        assert!(matches!(
            private_answer_read_by_other_user,
            Err(DomainError::Forbidden)
        ));

        let public_answer_form = form.change_answer_settings(
            AnswerSettings::default().change_visibility(AnswerVisibility::PUBLIC),
        );
        let public_answer_result =
            public_form_read_by(public_answer_form, Actor::from(other_user)).read_entry(entry);

        assert!(public_answer_result.is_ok());
    }

    #[test]
    fn public_form_with_group_restriction_is_readable_by_group_member_only() {
        let observer = user_group(10, "Observer");
        let form = sample_form().change_settings(
            FormSettings::new()
                .change_allowed_user_groups(AllowedUserGroups::new(vec![*observer.id()])),
        );
        let member = active_user_with_groups(Role::StandardUser, vec![observer]);
        let outsider = active_user(Role::StandardUser);

        assert!(form.can_read(&Actor::from(member)));
        assert!(!form.can_read(&Actor::from(outsider)));
        assert!(form.can_read(&Actor::from(active_user(Role::Administrator))));
    }

    #[test]
    fn try_accept_answer_respects_acceptance_period() {
        let user = active_user(Role::StandardUser);
        let actor = Actor::from(user.clone());
        let submitter = AnswerSubmitter::try_new(user, None, Utc::now()).unwrap();
        let form = sample_form().change_answer_settings(
            AnswerSettings::default().change_acceptance_period(
                AnswerAcceptancePeriod::try_new(
                    Some(Utc::now() - Duration::days(2)),
                    Some(Utc::now() - Duration::days(1)),
                )
                .unwrap(),
            ),
        );

        let result = public_form_read_by(form.clone(), actor).try_accept_answer(
            submitter,
            AnswerTitle::new(None),
            sample_posted_answers(&form),
        );

        assert!(matches!(result, Err(DomainError::Forbidden)));
    }
}
