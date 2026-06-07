use chrono::{DateTime, Utc};
#[cfg(test)]
use common::test_utils::{arbitrary_date_time, arbitrary_opt_date_time};
use derive_getters::Getters;
use deriving_via::DerivingVia;
use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;
#[cfg(test)]
use proptest::{collection::SizeRange, prelude::*, strategy::BoxedStrategy};
#[cfg(test)]
use proptest_derive::Arbitrary;
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, de};
use strum_macros::{Display, EnumString};
use types::non_empty_string::NonEmptyString;

pub use crate::form::question::models::{Question, QuestionSet};

use crate::{
    form::answer::models::{AnswerAuthor, AnswerEntry, AnswerTitle, PostedAnswerContents},
    types::authorization_guard::{
        Allowed, AuthorizationGuardDefinitions, AuthorizationRole, Create, Read, SelfGuarded,
        Update,
    },
    user::models::{Actor, Role::Administrator, User, UserId},
};

fn is_administrator(actor: &Actor) -> bool {
    matches!(actor, Actor::User(User::ActiveUser(user)) if user.role() == &Administrator)
}

pub type FormId = types::Id<ActiveForm>;
pub type FormLabelId = types::Id<FormLabel>;

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Clone, DerivingVia, Debug, PartialOrd, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize, Deserialize)]
pub struct FormTitle(NonEmptyString);

impl FormTitle {
    pub fn new(title: NonEmptyString) -> Self {
        Self(title)
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
pub struct FormSettings {
    #[serde(default)]
    webhook_url: WebhookUrl,
    #[serde(default)]
    visibility: Visibility,
}

impl FormSettings {
    pub fn new() -> Self {
        Self {
            webhook_url: WebhookUrl::try_new(None).unwrap(),
            visibility: Visibility::PUBLIC,
        }
    }

    pub fn webhook_url(&self, actor: &Actor) -> Result<&WebhookUrl, DomainError> {
        if is_administrator(actor) {
            Ok(&self.webhook_url)
        } else {
            Err(DomainError::Forbidden)
        }
    }

    pub fn visibility(&self) -> &Visibility {
        &self.visibility
    }

    pub fn change_webhook_url(self, webhook_url: WebhookUrl) -> Self {
        Self {
            webhook_url,
            ..self
        }
    }

    pub fn change_visibility(self, visibility: Visibility) -> Self {
        Self { visibility, ..self }
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Clone, DerivingVia, Default, Debug, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize(via: Option::<NonEmptyString>), Deserialize(via: Option::<NonEmptyString>
))]
pub struct WebhookUrl(Option<NonEmptyString>);

impl WebhookUrl {
    pub fn try_new(url: Option<NonEmptyString>) -> Result<Self, DomainError> {
        if let Some(url) = &url {
            let regex = Regex::new("https://discord.com/api/webhooks/.*").unwrap();
            if !regex.is_match(url) {
                return Err(DomainError::InvalidWebhookUrl);
            }
        }

        Ok(Self(url))
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(
    Serialize, Deserialize, Debug, EnumString, Display, Copy, Clone, Default, PartialOrd, PartialEq,
)]
pub enum Visibility {
    PUBLIC,
    #[default]
    PRIVATE,
}

impl TryFrom<String> for Visibility {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        use std::str::FromStr;
        Self::from_str(&value).map_err(Into::into)
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Clone, DerivingVia, Default, Debug, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize(via: Option::<NonEmptyString>), Deserialize(via: Option::<NonEmptyString>
))]
pub struct DefaultAnswerTitle(Option<NonEmptyString>);

impl DefaultAnswerTitle {
    pub fn new(title: Option<NonEmptyString>) -> Self {
        Self(title)
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(
    Serialize, Deserialize, Debug, EnumString, Display, Copy, Clone, Default, PartialOrd, PartialEq,
)]
pub enum AnswerVisibility {
    PUBLIC,
    #[default]
    PRIVATE,
}

impl TryFrom<String> for AnswerVisibility {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        use std::str::FromStr;
        Self::from_str(&value).map_err(Into::into)
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Getters, Clone, Default, Debug, PartialEq)]
pub struct ResponsePeriod {
    #[cfg_attr(test, proptest(strategy = "arbitrary_opt_date_time()"))]
    #[serde(default)]
    start_at: Option<DateTime<Utc>>,
    #[cfg_attr(test, proptest(strategy = "arbitrary_opt_date_time()"))]
    #[serde(default)]
    end_at: Option<DateTime<Utc>>,
}

impl ResponsePeriod {
    pub fn try_new(
        start_at: Option<DateTime<Utc>>,
        end_at: Option<DateTime<Utc>>,
    ) -> Result<Self, DomainError> {
        match (start_at, end_at) {
            (Some(start_at), Some(end_at)) if start_at > end_at => {
                Err(DomainError::InvalidResponsePeriod)
            }
            _ => Ok(Self { start_at, end_at }),
        }
    }

    pub fn is_within_period(&self, now: DateTime<Utc>) -> bool {
        if let Some(start_at) = self.start_at
            && start_at > now
        {
            return false;
        }
        if let Some(end_at) = self.end_at
            && end_at < now
        {
            return false;
        }
        true
    }
}

/// フォームの回答にまつわる設定をまとめた値オブジェクトです。
///
/// 回答の公開範囲・受付期間・仮回答可否・デフォルトタイトルといった「ポリシー」を保持し、
/// [`AnswerEntry`] の閲覧可否 ([`Self::can_read_entry`]) や新規受理 ([`Self::try_accept_answer`])
/// を判断します。この値オブジェクトは [`ActiveForm`] が所有します。
#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Getters, Clone, Default, Debug, PartialEq)]
pub struct AnswerSettings {
    default_answer_title: DefaultAnswerTitle,
    visibility: AnswerVisibility,
    response_period: ResponsePeriod,
    allow_temporary_answers: bool,
}

impl AnswerSettings {
    pub fn new(
        default_answer_title: DefaultAnswerTitle,
        visibility: AnswerVisibility,
        response_period: ResponsePeriod,
        allow_temporary_answers: bool,
    ) -> Self {
        Self {
            default_answer_title,
            visibility,
            response_period,
            allow_temporary_answers,
        }
    }

    pub fn change_default_answer_title(self, default_answer_title: DefaultAnswerTitle) -> Self {
        Self {
            default_answer_title,
            ..self
        }
    }

    pub fn change_visibility(self, visibility: AnswerVisibility) -> Self {
        Self { visibility, ..self }
    }

    pub fn change_response_period(self, response_period: ResponsePeriod) -> Self {
        Self {
            response_period,
            ..self
        }
    }

    pub fn change_allow_temporary_answers(self, allow_temporary_answers: bool) -> Self {
        Self {
            allow_temporary_answers,
            ..self
        }
    }

    /// `author` / `actor` の組み合わせと受付期間・仮回答可否から、新しい [`AnswerEntry`] を
    /// 受理してよいかを判断し、受理できる場合のみ [`AnswerEntry`] を生成します。
    /// `actor` が `author` として回答を作成してよいかを、受付期間と一時回答の可否から判定する。
    pub(crate) fn can_accept_answer(&self, author: &AnswerAuthor, actor: &Actor) -> bool {
        let is_within_period = self.response_period.is_within_period(Utc::now());

        match (author, actor) {
            (AnswerAuthor::AuthenticatedUser(user_id), Actor::User(User::ActiveUser(user))) => {
                *user_id == *user.id() && (is_within_period || user.role() == &Administrator)
            }
            (AnswerAuthor::TemporaryUser(_), Actor::User(User::TemporaryUser(_))) => {
                self.allow_temporary_answers && is_within_period
            }
            _ => false,
        }
    }

    pub fn try_accept_answer(
        &self,
        form_id: FormId,
        author: AnswerAuthor,
        actor: &Actor,
        title: AnswerTitle,
        posted_answers: PostedAnswerContents,
    ) -> Result<AnswerEntry, DomainError> {
        if !self.can_accept_answer(&author, actor) {
            return Err(DomainError::Forbidden);
        }

        Ok(AnswerEntry::new(form_id, author, title, posted_answers))
    }

    /// `actor` が `entry` を閲覧できるかどうかを、回答の公開範囲をもとに判断します。
    pub fn can_read_entry(&self, entry: &AnswerEntry, actor: &Actor) -> bool {
        match actor {
            Actor::User(User::ActiveUser(user)) => {
                entry.author().authenticated_user_id() == Some(*user.id())
                    || self.visibility == AnswerVisibility::PUBLIC
                    || user.role() == &Administrator
            }
            Actor::System => true,
            _ => false,
        }
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

#[derive(Serialize, Clone, DerivingVia, Default, Debug, PartialEq)]
#[deriving(IntoInner)]
pub struct FormLabelIdSet(Vec<FormLabelId>);

impl FormLabelIdSet {
    pub fn try_new(label_ids: Vec<FormLabelId>) -> Result<Self, DomainError> {
        if label_ids
            .iter()
            .enumerate()
            .any(|(index, label_id)| label_ids[..index].contains(label_id))
        {
            return Err(DomainError::InvalidEntity {
                message: "form label ids must be unique within a form".to_string(),
            });
        }

        Ok(Self(label_ids))
    }

    pub fn empty() -> Self {
        Self(Vec::new())
    }

    pub fn as_slice(&self) -> &[FormLabelId] {
        &self.0
    }
}

impl<'de> Deserialize<'de> for FormLabelIdSet {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<FormLabelId>::deserialize(deserializer)
            .and_then(|value| FormLabelIdSet::try_new(value).map_err(de::Error::custom))
    }
}

#[cfg(test)]
impl Arbitrary for FormLabelIdSet {
    type Parameters = (SizeRange, <FormLabelId as Arbitrary>::Parameters);
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(args: Self::Parameters) -> Self::Strategy {
        Vec::<FormLabelId>::arbitrary_with(args)
            .prop_filter_map("unique form label ids", |value| {
                FormLabelIdSet::try_new(value).ok()
            })
            .boxed()
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
    label_ids: FormLabelIdSet,
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
            label_ids: FormLabelIdSet::empty(),
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

    pub fn replace_label_ids(self, label_ids: FormLabelIdSet) -> Self {
        Self { label_ids, ..self }
    }

    pub fn archive(self, archived_at: DateTime<Utc>, archived_by: UserId) -> ArchivedForm {
        ArchivedForm::new(self, archived_at, archived_by)
    }
}

impl Allowed<ActiveForm, Read> {
    /// 回答にまつわるポリシー ([`AnswerSettings`]) に委譲して、新しい [`AnswerEntry`] を
    /// 受理してよいかを判断し、認可済みの [`Allowed<AnswerEntry, Create>`] を返します。
    pub fn try_accept_answer(
        &self,
        author: AnswerAuthor,
        title: AnswerTitle,
        posted_answers: PostedAnswerContents,
    ) -> Result<Allowed<AnswerEntry, Create>, DomainError> {
        let entry = self.value().answer_settings.try_accept_answer(
            *self.value().id(),
            author,
            self.actor(),
            title,
            posted_answers,
        )?;
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

#[derive(Serialize, Deserialize, Getters, Clone, Debug, PartialEq)]
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

    /// [`ArchivedForm`] の各フィールドを指定して再構築します。
    ///
    /// データベースから復元したデータなど、通常のアーカイブ操作を経ずに
    /// [`ArchivedForm`] を組み立てる場合に使用します。
    pub fn from_persisted(
        form: ActiveForm,
        archived_at: DateTime<Utc>,
        archived_by: UserId,
    ) -> Self {
        Self::new(form, archived_at, archived_by)
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
    /// 作成権限は以下の条件を満たしている場合に与えられます。
    /// - [`actor`] が [`Administrator`] である場合
    ///
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::models::ActiveForm,
    ///     types::authorization_guard::AuthorizationGuardDefinitions,
    ///     user::models::{ActiveUser, Actor, Role, User},
    /// };
    /// use uuid::Uuid;
    /// use domain::form::models::{FormDescription, FormTitle};
    ///
    /// let administrator: Actor = User::ActiveUser(ActiveUser::new(
    ///     "administrator".to_string(),
    ///     Uuid::new_v4().into(),
    ///     Role::Administrator,
    /// )).into();
    ///
    /// let standard_user: Actor = User::ActiveUser(ActiveUser::new(
    ///     "standard_user".to_string(),
    ///     Uuid::new_v4().into(),
    ///     Role::StandardUser,
    /// )).into();
    ///
    ///
    /// let form = ActiveForm::new(
    ///     FormTitle::new("テストフォーム".to_string().try_into().unwrap()),
    ///     FormDescription::new(String::from("")),
    ///     domain::form::models::QuestionSet::try_new(
    ///         types::non_empty_vec::NonEmptyVec::try_new(vec![
    ///             domain::form::question::models::Question::new_text(
    ///                 "q".to_string().try_into().unwrap(),
    ///                 0,
    ///                 "Q".to_string().try_into().unwrap(),
    ///                 None,
    ///                 true,
    ///             ).unwrap(),
    ///         ]).unwrap(),
    ///     ).unwrap(),
    /// );
    ///
    /// assert!(form.can_create(&administrator));
    /// assert!(!form.can_create(&standard_user));
    /// ```
    fn can_create(&self, actor: &Actor) -> bool {
        is_administrator(actor)
    }

    /// [`ActiveForm`] の読み取り権限があるかどうかを判定します。
    ///
    /// 読み取り権限は以下の条件のどちらかを満たしている場合に与えられます。
    /// - [`actor`] が [`Administrator`] である場合
    /// - [`ActiveForm`] が全体公開されている場合
    ///
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::models::{ActiveForm, FormSettings},
    ///     types::authorization_guard::AuthorizationGuardDefinitions,
    ///     user::models::{ActiveUser, Actor, Role, User},
    /// };
    /// use uuid::Uuid;
    /// use domain::form::models::{
    ///     FormDescription, FormTitle, Visibility
    /// };
    ///
    /// let administrator: Actor = User::ActiveUser(ActiveUser::new(
    ///     "administrator".to_string(),
    ///     Uuid::new_v4().into(),
    ///     Role::Administrator,
    /// )).into();
    ///
    /// let standard_user: Actor = User::ActiveUser(ActiveUser::new(
    ///     "standard_user".to_string(),
    ///     Uuid::new_v4().into(),
    ///     Role::StandardUser,
    /// )).into();
    ///
    ///
    /// let sample_questions = || domain::form::models::QuestionSet::try_new(
    ///     types::non_empty_vec::NonEmptyVec::try_new(vec![
    ///         domain::form::question::models::Question::new_text(
    ///             "q".to_string().try_into().unwrap(),
    ///             0,
    ///             "Q".to_string().try_into().unwrap(),
    ///             None,
    ///             true,
    ///         ).unwrap(),
    ///     ]).unwrap(),
    /// ).unwrap();
    ///
    /// let private_form = ActiveForm::new(
    ///     FormTitle::new("非公開フォーム".to_string().try_into().unwrap()),
    ///     FormDescription::new(String::from("")),
    ///     sample_questions(),
    /// ).change_settings(FormSettings::new().change_visibility(Visibility::PRIVATE));
    ///
    ///  let public_form = ActiveForm::new(
    ///     FormTitle::new("公開フォーム".to_string().try_into().unwrap()),
    ///     FormDescription::new(String::from("")),
    ///     sample_questions(),
    /// ).change_settings(FormSettings::new().change_visibility(Visibility::PUBLIC));
    ///
    /// assert!(private_form.can_read(&administrator));
    /// assert!(!private_form.can_read(&standard_user));
    /// assert!(public_form.can_read(&administrator));
    /// assert!(public_form.can_read(&standard_user));
    /// ```
    fn can_read(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::System)
            || self.settings.visibility == Visibility::PUBLIC
            || is_administrator(actor)
    }

    /// [`ActiveForm`] の更新権限があるかどうかを判定します。
    ///
    /// 更新権限は以下の条件を満たしている場合に与えられます。
    /// - [`actor`] が [`Administrator`] である場合
    ///
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::models::ActiveForm,
    ///     types::authorization_guard::AuthorizationGuardDefinitions,
    ///     user::models::{ActiveUser, Actor, Role, User},
    /// };
    /// use uuid::Uuid;
    /// use domain::form::models::{FormDescription, FormLabelIdSet, FormTitle};
    ///
    /// let administrator: Actor = User::ActiveUser(ActiveUser::new(
    ///     "administrator".to_string(),
    ///     Uuid::new_v4().into(),
    ///     Role::Administrator,
    /// )).into();
    ///
    /// let standard_user: Actor = User::ActiveUser(ActiveUser::new(
    ///     "standard_user".to_string(),
    ///     Uuid::new_v4().into(),
    ///     Role::StandardUser,
    /// )).into();
    ///
    ///
    /// let form = ActiveForm::new(
    ///     FormTitle::new("テストフォーム".to_string().try_into().unwrap()),
    ///     FormDescription::new(String::from("")),
    ///     domain::form::models::QuestionSet::try_new(
    ///         types::non_empty_vec::NonEmptyVec::try_new(vec![
    ///             domain::form::question::models::Question::new_text(
    ///                 "q".to_string().try_into().unwrap(),
    ///                 0,
    ///                 "Q".to_string().try_into().unwrap(),
    ///                 None,
    ///                 true,
    ///             ).unwrap(),
    ///         ]).unwrap(),
    ///     ).unwrap(),
    /// );
    ///
    /// assert!(form.can_update(&administrator));
    /// assert!(!form.can_update(&standard_user));
    /// ```
    fn can_update(&self, actor: &Actor) -> bool {
        is_administrator(actor)
    }

    /// [`ActiveForm`] の削除権限があるかどうかを判定します。
    ///
    /// 削除権限は以下の条件を満たしている場合に与えられます。
    /// - [`actor`] が [`Administrator`] である場合
    ///
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::models::ActiveForm,
    ///     types::authorization_guard::AuthorizationGuardDefinitions,
    ///     user::models::{ActiveUser, Actor, Role, User},
    /// };
    /// use uuid::Uuid;
    /// use domain::form::models::{FormDescription, FormLabelIdSet, FormTitle};
    ///
    /// let administrator: Actor = User::ActiveUser(ActiveUser::new(
    ///     "administrator".to_string(),
    ///     Uuid::new_v4().into(),
    ///     Role::Administrator,
    /// )).into();
    ///
    /// let standard_user: Actor = User::ActiveUser(ActiveUser::new(
    ///     "standard_user".to_string(),
    ///     Uuid::new_v4().into(),
    ///     Role::StandardUser,
    /// )).into();
    ///
    ///
    /// let form = ActiveForm::new(
    ///     FormTitle::new("テストフォーム".to_string().try_into().unwrap()),
    ///     FormDescription::new(String::from("")),
    ///     domain::form::models::QuestionSet::try_new(
    ///         types::non_empty_vec::NonEmptyVec::try_new(vec![
    ///             domain::form::question::models::Question::new_text(
    ///                 "q".to_string().try_into().unwrap(),
    ///                 0,
    ///                 "Q".to_string().try_into().unwrap(),
    ///                 None,
    ///                 true,
    ///             ).unwrap(),
    ///         ]).unwrap(),
    ///     ).unwrap(),
    /// );
    ///
    /// assert!(!form.can_delete(&administrator));
    /// assert!(!form.can_delete(&standard_user));
    /// ```
    fn can_delete(&self, _actor: &Actor) -> bool {
        false
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Clone, DerivingVia, Debug, PartialOrd, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize(via: NonEmptyString), Deserialize(via: NonEmptyString))]
pub struct FormLabelName(NonEmptyString);

impl FormLabelName {
    pub fn new(name: NonEmptyString) -> Self {
        Self(name)
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(UnsafeFromRawParts, Serialize, Deserialize, Getters, Debug, PartialEq)]
pub struct FormLabel {
    id: FormLabelId,
    name: FormLabelName,
}

impl FormLabel {
    pub fn new(name: FormLabelName) -> Self {
        Self {
            id: FormLabelId::new(),
            name,
        }
    }

    pub fn renamed(&self, name: FormLabelName) -> Self {
        Self { id: self.id, name }
    }
}

impl AuthorizationRole for FormLabel {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for FormLabel {
    /// [`FormLabel`] の作成権限があるかどうかを判定します。
    ///
    /// 作成権限は以下の条件を満たしている場合に与えられます。
    /// - [`actor`] が [`Administrator`] である場合
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::models::{FormLabel, FormLabelName},
    ///     types::authorization_guard::AuthorizationGuardDefinitions,
    ///     user::models::{ActiveUser, Actor, Role, User},
    /// };
    /// use types::non_empty_string::NonEmptyString;
    /// use uuid::Uuid;
    /// use domain::form::models::AnswerSettings;
    ///
    /// let administrator: Actor = User::ActiveUser(ActiveUser::new(
    ///     "administrator".to_string(),
    ///     Uuid::new_v4().into(),
    ///     Role::Administrator,
    /// )).into();
    ///
    /// let standard_user: Actor = User::ActiveUser(ActiveUser::new(
    ///     "standard_user".to_string(),
    ///     Uuid::new_v4().into(),
    ///     Role::StandardUser,
    /// )).into();
    ///
    /// let form_label = FormLabel::new(FormLabelName::new(
    ///     NonEmptyString::try_new("テストラベル".to_string()).unwrap(),
    /// ));
    ///
    /// assert!(form_label.can_create(&administrator));
    /// assert!(!form_label.can_create(&standard_user));
    /// ```
    fn can_create(&self, actor: &Actor) -> bool {
        is_administrator(actor)
    }

    /// [`FormLabel`] の読み取り権限があるかどうかを判定します。
    ///
    /// 読み取り権限はすべてのユーザーに与えられます。
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::models::{FormLabel, FormLabelName},
    ///     types::authorization_guard::AuthorizationGuardDefinitions,
    ///     user::models::{ActiveUser, Actor, Role, User},
    /// };
    /// use types::non_empty_string::NonEmptyString;
    /// use uuid::Uuid;
    /// use domain::form::models::AnswerSettings;
    ///
    /// let administrator: Actor = User::ActiveUser(ActiveUser::new(
    ///     "administrator".to_string(),
    ///     Uuid::new_v4().into(),
    ///     Role::Administrator,
    /// )).into();
    ///
    /// let standard_user: Actor = User::ActiveUser(ActiveUser::new(
    ///     "standard_user".to_string(),
    ///     Uuid::new_v4().into(),
    ///     Role::StandardUser,
    /// )).into();
    ///
    /// let form_label = FormLabel::new(FormLabelName::new(
    ///     NonEmptyString::try_new("テストラベル".to_string()).unwrap(),
    /// ));
    ///
    /// assert!(form_label.can_read(&administrator));
    /// assert!(form_label.can_read(&standard_user));
    /// ```
    fn can_read(&self, _actor: &Actor) -> bool {
        true
    }

    /// [`FormLabel`] の更新権限があるかどうかを判定します。
    ///
    /// 更新権限は以下の条件を満たしている場合に与えられます。
    /// - [`actor`] が [`Administrator`] である場合
    ///
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::models::{FormLabel, FormLabelName},
    ///     types::authorization_guard::AuthorizationGuardDefinitions,
    ///     user::models::{ActiveUser, Actor, Role, User},
    /// };
    /// use types::non_empty_string::NonEmptyString;
    /// use uuid::Uuid;
    /// use domain::form::models::AnswerSettings;
    ///
    /// let administrator: Actor = User::ActiveUser(ActiveUser::new(
    ///     "administrator".to_string(),
    ///     Uuid::new_v4().into(),
    ///     Role::Administrator,
    /// )).into();
    ///
    /// let standard_user: Actor = User::ActiveUser(ActiveUser::new(
    ///     "standard_user".to_string(),
    ///     Uuid::new_v4().into(),
    ///     Role::StandardUser,
    /// )).into();
    ///
    /// let form_label = FormLabel::new(FormLabelName::new(
    ///     NonEmptyString::try_new("テストラベル".to_string()).unwrap(),
    /// ));
    ///
    /// assert!(form_label.can_update(&administrator));
    /// assert!(!form_label.can_update(&standard_user));
    /// ```
    fn can_update(&self, actor: &Actor) -> bool {
        is_administrator(actor)
    }

    /// [`FormLabel`] の削除権限があるかどうかを判定します。
    ///
    /// 削除権限は以下の条件を満たしている場合に与えられます。
    /// - [`actor`] が [`Administrator`] である場合
    ///
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::models::{FormLabel, FormLabelName},
    ///     types::authorization_guard::AuthorizationGuardDefinitions,
    ///     user::models::{ActiveUser, Actor, Role, User},
    /// };
    /// use types::non_empty_string::NonEmptyString;
    /// use uuid::Uuid;
    /// use domain::form::models::AnswerSettings;
    ///
    /// let administrator: Actor = User::ActiveUser(ActiveUser::new(
    ///     "administrator".to_string(),
    ///     Uuid::new_v4().into(),
    ///     Role::Administrator,
    /// )).into();
    ///
    /// let standard_user: Actor = User::ActiveUser(ActiveUser::new(
    ///     "standard_user".to_string(),
    ///     Uuid::new_v4().into(),
    ///     Role::StandardUser,
    /// )).into();
    ///
    /// let form_label = FormLabel::new(FormLabelName::new(
    ///     NonEmptyString::try_new("テストラベル".to_string()).unwrap(),
    /// ));
    ///
    /// assert!(form_label.can_delete(&administrator));
    /// assert!(!form_label.can_delete(&standard_user));
    /// ```
    fn can_delete(&self, actor: &Actor) -> bool {
        is_administrator(actor)
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;
    use crate::form::question::models::{Question, QuestionId, QuestionType};
    use crate::user::models::Role;
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

    fn answer_settings(
        allow_temporary_answers: bool,
        response_period: ResponsePeriod,
    ) -> AnswerSettings {
        AnswerSettings::new(
            DefaultAnswerTitle::new(None),
            AnswerVisibility::PRIVATE,
            response_period,
            allow_temporary_answers,
        )
    }

    fn active_user(role: Role) -> crate::user::models::ActiveUser {
        crate::user::models::ActiveUser::new("user".to_string(), UserId::from(Uuid::new_v4()), role)
    }

    fn answer_entry(author: AnswerAuthor) -> AnswerEntry {
        AnswerEntry::new(
            FormId::new(),
            author,
            AnswerTitle::new(None),
            PostedAnswerContents::try_new(&[], Vec::new()).unwrap(),
        )
    }

    fn empty_posted_answers() -> PostedAnswerContents {
        PostedAnswerContents::try_new(&[], vec![]).unwrap()
    }

    #[test]
    fn temporary_answer_creation_requires_allow_flag() {
        let settings = answer_settings(false, ResponsePeriod::try_new(None, None).unwrap());
        let author = AnswerAuthor::TemporaryUser(crate::user::models::TemporaryUser::new(
            "guest".to_string(),
            "contact".to_string(),
        ));
        let actor = Actor::from(crate::user::models::TemporaryUser::new(
            "guest".to_string(),
            "contact".to_string(),
        ));

        assert!(
            settings
                .try_accept_answer(
                    FormId::new(),
                    author,
                    &actor,
                    AnswerTitle::new(None),
                    empty_posted_answers()
                )
                .is_err()
        );
    }

    #[test]
    fn temporary_answer_creation_succeeds_when_allowed_and_within_period() {
        let settings = answer_settings(true, ResponsePeriod::try_new(None, None).unwrap());
        let author = AnswerAuthor::TemporaryUser(crate::user::models::TemporaryUser::new(
            "guest".to_string(),
            "contact".to_string(),
        ));
        let actor = Actor::from(crate::user::models::TemporaryUser::new(
            "guest".to_string(),
            "contact".to_string(),
        ));

        assert!(
            settings
                .try_accept_answer(
                    FormId::new(),
                    author,
                    &actor,
                    AnswerTitle::new(None),
                    empty_posted_answers()
                )
                .is_ok()
        );
    }

    #[test]
    fn temporary_answer_creation_respects_response_period() {
        let settings = answer_settings(
            true,
            ResponsePeriod::try_new(
                Some(Utc::now() - Duration::days(2)),
                Some(Utc::now() - Duration::days(1)),
            )
            .unwrap(),
        );
        let author = AnswerAuthor::TemporaryUser(crate::user::models::TemporaryUser::new(
            "guest".to_string(),
            "contact".to_string(),
        ));
        let actor = Actor::from(crate::user::models::TemporaryUser::new(
            "guest".to_string(),
            "contact".to_string(),
        ));

        assert!(
            settings
                .try_accept_answer(
                    FormId::new(),
                    author,
                    &actor,
                    AnswerTitle::new(None),
                    empty_posted_answers()
                )
                .is_err()
        );
    }

    #[test]
    fn private_entry_is_readable_by_its_author() {
        let author = active_user(Role::StandardUser);
        let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
        let settings = answer_settings(false, ResponsePeriod::try_new(None, None).unwrap());

        assert!(settings.can_read_entry(&entry, &Actor::from(author)));
    }

    #[test]
    fn private_entry_is_not_readable_by_other_standard_user() {
        let author = active_user(Role::StandardUser);
        let other = active_user(Role::StandardUser);
        let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
        let settings = answer_settings(false, ResponsePeriod::try_new(None, None).unwrap());

        assert!(!settings.can_read_entry(&entry, &Actor::from(other)));
    }

    #[test]
    fn private_entry_is_readable_by_administrator() {
        let author = active_user(Role::StandardUser);
        let administrator = active_user(Role::Administrator);
        let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
        let settings = answer_settings(false, ResponsePeriod::try_new(None, None).unwrap());

        assert!(settings.can_read_entry(&entry, &Actor::from(administrator)));
    }

    #[test]
    fn public_entry_is_readable_by_other_standard_user() {
        let author = active_user(Role::StandardUser);
        let other = active_user(Role::StandardUser);
        let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
        let settings = AnswerSettings::new(
            DefaultAnswerTitle::new(None),
            AnswerVisibility::PUBLIC,
            ResponsePeriod::try_new(None, None).unwrap(),
            false,
        );

        assert!(settings.can_read_entry(&entry, &Actor::from(other)));
    }

    #[test]
    fn form_label_id_set_allows_empty_ids() {
        let label_ids = FormLabelIdSet::try_new(vec![]).unwrap();

        assert!(label_ids.as_slice().is_empty());
    }

    #[test]
    fn form_label_id_set_allows_unique_ids() {
        let first = FormLabelId::new();
        let second = FormLabelId::new();
        let label_ids = FormLabelIdSet::try_new(vec![first, second]).unwrap();

        assert_eq!(label_ids.as_slice(), &[first, second]);
    }

    #[test]
    fn form_label_id_set_rejects_duplicate_ids() {
        let label_id = FormLabelId::new();
        let result = FormLabelIdSet::try_new(vec![label_id, label_id]);

        assert!(matches!(result, Err(DomainError::InvalidEntity { .. })));
    }

    #[test]
    fn form_label_id_set_deserialize_rejects_duplicate_ids() {
        let label_id = FormLabelId::new();
        let serialized = serde_json::to_string(&vec![label_id, label_id]).unwrap();
        let result = serde_json::from_str::<FormLabelIdSet>(&serialized);

        assert!(result.is_err());
    }

    #[test]
    fn active_form_new_has_empty_label_ids() {
        let form = ActiveForm::new(
            FormTitle::new("Form".to_string().try_into().unwrap()),
            FormDescription::new("description".to_string()),
            sample_question_set(),
        );

        assert!(form.label_ids().as_slice().is_empty());
    }

    #[test]
    fn active_form_replace_label_ids_replaces_ids() {
        let label_id = FormLabelId::new();
        let form = ActiveForm::new(
            FormTitle::new("Form".to_string().try_into().unwrap()),
            FormDescription::new("description".to_string()),
            sample_question_set(),
        )
        .replace_label_ids(FormLabelIdSet::try_new(vec![label_id]).unwrap());

        assert_eq!(form.label_ids().as_slice(), &[label_id]);
    }
}
