use chrono::{DateTime, Utc};
#[cfg(test)]
use common::test_utils::arbitrary_date_time;
use derive_getters::Getters;
use deriving_via::DerivingVia;
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
    form::answer::settings::models::{
        AnswerSettings, AnswerVisibility, DefaultAnswerTitle, ResponsePeriod,
    },
    types::authorization_guard::AuthorizationGuardDefinitions,
    user::models::{Role::Administrator, User, UserId},
};

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
#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
pub struct FormSettings {
    #[serde(default)]
    webhook_url: WebhookUrl,
    #[serde(default)]
    visibility: Visibility,
    #[serde(default)]
    allow_temporary_answers: bool,
    answer_settings: AnswerSettings,
}

impl FormSettings {
    pub fn new() -> Self {
        Self {
            webhook_url: WebhookUrl::try_new(None).unwrap(),
            visibility: Visibility::PUBLIC,
            allow_temporary_answers: false,
            answer_settings: AnswerSettings::new(
                DefaultAnswerTitle::new(None),
                AnswerVisibility::PRIVATE,
                ResponsePeriod::try_new(None, None).unwrap(),
            ),
        }
    }

    pub fn webhook_url(&self, user: &User) -> Result<&WebhookUrl, DomainError> {
        if user.role == Administrator {
            Ok(&self.webhook_url)
        } else {
            Err(DomainError::Forbidden)
        }
    }

    pub fn visibility(&self) -> &Visibility {
        &self.visibility
    }

    pub fn answer_settings(&self) -> &AnswerSettings {
        &self.answer_settings
    }

    pub fn allow_temporary_answers(&self) -> bool {
        self.allow_temporary_answers
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

    pub fn change_allow_temporary_answers(self, allow_temporary_answers: bool) -> Self {
        Self {
            allow_temporary_answers,
            ..self
        }
    }

    pub fn change_answer_settings(self, answer_settings: AnswerSettings) -> Self {
        Self {
            answer_settings,
            ..self
        }
    }

    pub fn from_raw_parts(
        response_period: ResponsePeriod,
        webhook_url: WebhookUrl,
        default_answer_title: DefaultAnswerTitle,
        visibility: Visibility,
        allow_temporary_answers: bool,
        answer_visibility: AnswerVisibility,
    ) -> Self {
        Self {
            webhook_url,
            visibility,
            allow_temporary_answers,
            answer_settings: AnswerSettings::new(
                default_answer_title,
                answer_visibility,
                response_period,
            ),
        }
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
#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq)]
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

    pub fn from_raw_parts(created_at: DateTime<Utc>, updated_at: DateTime<Utc>) -> Self {
        Self {
            created_at,
            updated_at,
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
#[derive(Serialize, Deserialize, Getters, Clone, Debug, PartialEq)]
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

    pub fn change_questions(self, questions: QuestionSet) -> Self {
        Self { questions, ..self }
    }

    pub fn replace_label_ids(self, label_ids: FormLabelIdSet) -> Self {
        Self { label_ids, ..self }
    }

    pub fn from_raw_parts(
        id: FormId,
        title: FormTitle,
        description: FormDescription,
        metadata: FormMeta,
        settings: FormSettings,
        questions: QuestionSet,
        label_ids: FormLabelIdSet,
    ) -> Self {
        Self {
            id,
            title,
            description,
            metadata,
            settings,
            questions,
            label_ids,
        }
    }

    pub fn archive(self, archived_at: DateTime<Utc>, archived_by: UserId) -> ArchivedForm {
        ArchivedForm::new(self, archived_at, archived_by)
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

impl AuthorizationGuardDefinitions for ArchivedForm {
    fn can_create(&self, actor: &User) -> bool {
        actor.role == Administrator
    }

    fn can_read(&self, actor: &User) -> bool {
        actor.role == Administrator
    }

    fn can_update(&self, actor: &User) -> bool {
        actor.role == Administrator
    }

    fn can_delete(&self, actor: &User) -> bool {
        actor.role == Administrator
    }
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
    ///     form::models::{ActiveForm, FormId, FormMeta, FormSettings},
    ///     types::authorization_guard::AuthorizationGuardDefinitions,
    ///     user::models::{Role, User},
    /// };
    /// use uuid::Uuid;
    /// use domain::form::models::{FormDescription, FormTitle};
    /// use domain::form::answer::settings::models::{AnswerVisibility, DefaultAnswerTitle, ResponsePeriod};
    /// use domain::form::models::{FormLabelIdSet, Visibility, WebhookUrl};
    ///
    /// let administrator = User {
    ///     name: "administrator".to_string(),
    ///     id: Uuid::new_v4().into(),
    ///     role: Role::Administrator,
    /// };
    ///
    /// let standard_user = User {
    ///     name: "standard_user".to_string(),
    ///     id: Uuid::new_v4().into(),
    ///     role: Role::StandardUser,
    /// };
    ///
    ///
    /// let form = ActiveForm::from_raw_parts(
    ///     FormId::new(),
    ///     FormTitle::new("テストフォーム".to_string().try_into().unwrap()),
    ///     FormDescription::new(String::from("")),
    ///     FormMeta::new(),
    ///     FormSettings::new(),
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
    ///     FormLabelIdSet::empty(),
    /// );
    ///
    /// assert!(form.can_create(&administrator));
    /// assert!(!form.can_create(&standard_user));
    fn can_create(&self, actor: &User) -> bool {
        actor.role == Administrator
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
    ///     user::models::{Role, User},
    /// };
    /// use uuid::Uuid;
    /// use domain::form::answer::settings::models::{AnswerVisibility, DefaultAnswerTitle, ResponsePeriod};
    /// use domain::form::models::{
    ///     FormDescription, FormId, FormMeta,
    ///     FormLabelIdSet, FormTitle, Visibility, WebhookUrl
    /// };
    ///
    /// let administrator = User {
    ///     name: "administrator".to_string(),
    ///     id: Uuid::new_v4().into(),
    ///     role: Role::Administrator,
    /// };
    ///
    /// let standard_user = User {
    ///     name: "standard_user".to_string(),
    ///     id: Uuid::new_v4().into(),
    ///     role: Role::StandardUser,
    /// };
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
    /// let private_form = ActiveForm::from_raw_parts(
    ///     FormId::new(),
    ///     FormTitle::new("非公開フォーム".to_string().try_into().unwrap()),
    ///     FormDescription::new(String::from("")),
    ///     FormMeta::new(),
    ///     FormSettings::from_raw_parts(
    ///         ResponsePeriod::try_new(None, None).unwrap(),
    ///         WebhookUrl::try_new(None).unwrap(),
    ///         DefaultAnswerTitle::new(None),
    ///         Visibility::PRIVATE,
    ///         false,
    ///         AnswerVisibility::PRIVATE
    ///     ),
    ///     sample_questions(),
    ///     FormLabelIdSet::empty(),
    /// );
    ///
    ///  let public_form = ActiveForm::from_raw_parts(
    ///     FormId::new(),
    ///     FormTitle::new("公開フォーム".to_string().try_into().unwrap()),
    ///     FormDescription::new(String::from("")),
    ///     FormMeta::new(),
    ///     FormSettings::from_raw_parts(
    ///         ResponsePeriod::try_new(None, None).unwrap(),
    ///         WebhookUrl::try_new(None).unwrap(),
    ///         DefaultAnswerTitle::new(None),
    ///         Visibility::PUBLIC,
    ///         false,
    ///         AnswerVisibility::PRIVATE
    ///     ),
    ///     sample_questions(),
    ///     FormLabelIdSet::empty(),
    /// );
    ///
    /// assert!(private_form.can_read(&administrator));
    /// assert!(!private_form.can_read(&standard_user));
    /// assert!(public_form.can_read(&administrator));
    /// assert!(public_form.can_read(&standard_user));
    fn can_read(&self, actor: &User) -> bool {
        self.settings.visibility == Visibility::PUBLIC || actor.role == Administrator
    }

    /// [`ActiveForm`] の更新権限があるかどうかを判定します。
    ///
    /// 更新権限は以下の条件を満たしている場合に与えられます。
    /// - [`actor`] が [`Administrator`] である場合
    ///
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::models::{ActiveForm, FormId, FormMeta, FormSettings},
    ///     types::authorization_guard::AuthorizationGuardDefinitions,
    ///     user::models::{Role, User},
    /// };
    /// use uuid::Uuid;
    /// use domain::form::models::{FormDescription, FormLabelIdSet, FormTitle};
    ///
    /// let administrator = User {
    ///     name: "administrator".to_string(),
    ///     id: Uuid::new_v4().into(),
    ///     role: Role::Administrator,
    /// };
    ///
    /// let standard_user = User {
    ///     name: "standard_user".to_string(),
    ///     id: Uuid::new_v4().into(),
    ///     role: Role::StandardUser,
    /// };
    ///
    ///
    /// let form = ActiveForm::from_raw_parts(
    ///     FormId::new(),
    ///     FormTitle::new("テストフォーム".to_string().try_into().unwrap()),
    ///     FormDescription::new(String::from("")),
    ///     FormMeta::new(),
    ///     FormSettings::new(),
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
    ///     FormLabelIdSet::empty(),
    /// );
    ///
    /// assert!(form.can_update(&administrator));
    /// assert!(!form.can_update(&standard_user));
    fn can_update(&self, actor: &User) -> bool {
        actor.role == Administrator
    }

    /// [`ActiveForm`] の削除権限があるかどうかを判定します。
    ///
    /// 削除権限は以下の条件を満たしている場合に与えられます。
    /// - [`actor`] が [`Administrator`] である場合
    ///
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::models::{ActiveForm, FormId, FormMeta, FormSettings},
    ///     types::authorization_guard::AuthorizationGuardDefinitions,
    ///     user::models::{Role, User},
    /// };
    /// use uuid::Uuid;
    /// use domain::form::models::{FormDescription, FormLabelIdSet, FormTitle};
    ///
    /// let administrator = User {
    ///     name: "administrator".to_string(),
    ///     id: Uuid::new_v4().into(),
    ///     role: Role::Administrator,
    /// };
    ///
    /// let standard_user = User {
    ///     name: "standard_user".to_string(),
    ///     id: Uuid::new_v4().into(),
    ///     role: Role::StandardUser,
    /// };
    ///
    ///
    /// let form = ActiveForm::from_raw_parts(
    ///     FormId::new(),
    ///     FormTitle::new("テストフォーム".to_string().try_into().unwrap()),
    ///     FormDescription::new(String::from("")),
    ///     FormMeta::new(),
    ///     FormSettings::new(),
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
    ///     FormLabelIdSet::empty(),
    /// );
    ///
    /// assert!(!form.can_delete(&administrator));
    /// assert!(!form.can_delete(&standard_user));
    fn can_delete(&self, _actor: &User) -> bool {
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
#[derive(Serialize, Deserialize, Getters, Debug, PartialEq)]
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

    pub fn from_raw_parts(id: FormLabelId, name: FormLabelName) -> Self {
        Self { id, name }
    }
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
    ///     user::models::{Role, User},
    /// };
    /// use types::non_empty_string::NonEmptyString;
    /// use uuid::Uuid;
    ///
    /// let administrator = User {
    ///     name: "administrator".to_string(),
    ///     id: Uuid::new_v4().into(),
    ///     role: Role::Administrator,
    /// };
    ///
    /// let standard_user = User {
    ///     name: "standard_user".to_string(),
    ///     id: Uuid::new_v4().into(),
    ///     role: Role::StandardUser,
    /// };
    ///
    /// let form_label = FormLabel::new(FormLabelName::new(
    ///     NonEmptyString::try_new("テストラベル".to_string()).unwrap(),
    /// ));
    ///
    /// assert!(form_label.can_create(&administrator));
    /// assert!(!form_label.can_create(&standard_user));
    /// ```
    fn can_create(&self, actor: &User) -> bool {
        actor.role == Administrator
    }

    /// [`FormLabel`] の読み取り権限があるかどうかを判定します。
    ///
    /// 読み取り権限はすべてのユーザーに与えられます。
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::models::{FormLabel, FormLabelName},
    ///     types::authorization_guard::AuthorizationGuardDefinitions,
    ///     user::models::{Role, User},
    /// };
    /// use types::non_empty_string::NonEmptyString;
    /// use uuid::Uuid;
    ///
    /// let administrator = User {
    ///     name: "administrator".to_string(),
    ///     id: Uuid::new_v4().into(),
    ///     role: Role::Administrator,
    /// };
    ///
    /// let standard_user = User {
    ///     name: "standard_user".to_string(),
    ///     id: Uuid::new_v4().into(),
    ///     role: Role::StandardUser,
    /// };
    ///
    /// let form_label = FormLabel::new(FormLabelName::new(
    ///     NonEmptyString::try_new("テストラベル".to_string()).unwrap(),
    /// ));
    ///
    /// assert!(form_label.can_read(&administrator));
    /// assert!(form_label.can_read(&standard_user));
    /// ```
    fn can_read(&self, _actor: &User) -> bool {
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
    ///     user::models::{Role, User},
    /// };
    /// use types::non_empty_string::NonEmptyString;
    /// use uuid::Uuid;
    ///
    /// let administrator = User {
    ///     name: "administrator".to_string(),
    ///     id: Uuid::new_v4().into(),
    ///     role: Role::Administrator,
    /// };
    ///
    /// let standard_user = User {
    ///     name: "standard_user".to_string(),
    ///     id: Uuid::new_v4().into(),
    ///     role: Role::StandardUser,
    /// };
    ///
    /// let form_label = FormLabel::new(FormLabelName::new(
    ///     NonEmptyString::try_new("テストラベル".to_string()).unwrap(),
    /// ));
    ///
    /// assert!(form_label.can_update(&administrator));
    /// assert!(!form_label.can_update(&standard_user));
    /// ```
    fn can_update(&self, actor: &User) -> bool {
        actor.role == Administrator
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
    ///     user::models::{Role, User},
    /// };
    /// use types::non_empty_string::NonEmptyString;
    /// use uuid::Uuid;
    ///
    /// let administrator = User {
    ///     name: "administrator".to_string(),
    ///     id: Uuid::new_v4().into(),
    ///     role: Role::Administrator,
    /// };
    ///
    /// let standard_user = User {
    ///     name: "standard_user".to_string(),
    ///     id: Uuid::new_v4().into(),
    ///     role: Role::StandardUser,
    /// };
    ///
    /// let form_label = FormLabel::new(FormLabelName::new(
    ///     NonEmptyString::try_new("テストラベル".to_string()).unwrap(),
    /// ));
    ///
    /// assert!(form_label.can_delete(&administrator));
    /// assert!(!form_label.can_delete(&standard_user));
    /// ```
    fn can_delete(&self, actor: &User) -> bool {
        actor.role == Administrator
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form::question::models::{Question, QuestionId, QuestionType};
    use types::non_empty_vec::NonEmptyVec;
    use uuid::Uuid;

    fn sample_question_set() -> QuestionSet {
        QuestionSet::try_new(
            NonEmptyVec::try_new(vec![
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
                .unwrap(),
            ])
            .unwrap(),
        )
        .unwrap()
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
