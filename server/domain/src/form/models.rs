use chrono::{DateTime, Utc};
#[cfg(test)]
use common::test_utils::arbitrary_date_time;
use derive_getters::Getters;
use deriving_via::DerivingVia;
use errors::domain::DomainError;
#[cfg(test)]
use proptest_derive::Arbitrary;
use regex::Regex;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};
use types::non_empty_string::NonEmptyString;

pub use crate::form::question::models::{Question, QuestionSet};

use crate::{
    form::answer::settings::models::{
        AnswerSettings, AnswerVisibility, DefaultAnswerTitle, ResponsePeriod,
    },
    types::authorization_guard::AuthorizationGuardDefinitions,
    user::models::{Role::Administrator, User},
};

pub type FormId = types::Id<ActiveForm>;

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
    answer_settings: AnswerSettings,
}

impl FormSettings {
    pub fn new() -> Self {
        Self {
            webhook_url: WebhookUrl::try_new(None).unwrap(),
            visibility: Visibility::PUBLIC,
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

    pub fn change_webhook_url(self, webhook_url: WebhookUrl) -> Self {
        Self {
            webhook_url,
            ..self
        }
    }

    pub fn change_visibility(self, visibility: Visibility) -> Self {
        Self { visibility, ..self }
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
        answer_visibility: AnswerVisibility,
    ) -> Self {
        Self {
            webhook_url,
            visibility,
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

    pub fn from_raw_parts(
        id: FormId,
        title: FormTitle,
        description: FormDescription,
        metadata: FormMeta,
        settings: FormSettings,
        questions: QuestionSet,
    ) -> Self {
        Self {
            id,
            title,
            description,
            metadata,
            settings,
            questions,
        }
    }

    pub fn archive(self, archived_at: DateTime<Utc>, archived_by: User) -> ArchivedForm {
        unsafe { ArchivedForm::from_raw_parts(self, archived_at, archived_by) }
    }
}

#[derive(Serialize, Deserialize, Getters, Clone, Debug, PartialEq)]
pub struct ArchivedForm {
    form: ActiveForm,
    archived_at: DateTime<Utc>,
    archived_by: User,
}

impl ArchivedForm {
    /// [`ArchivedForm`] の各フィールドを指定して新しく作成します。
    ///
    /// # Safety
    /// この関数は [`ArchivedForm`] のバリデーションをスキップするため、
    /// データベースからすでに検証済みのデータを読み出すときなど、
    /// データの信頼性が保証されている場合にのみ使用してください。
    pub unsafe fn from_raw_parts(
        form: ActiveForm,
        archived_at: DateTime<Utc>,
        archived_by: User,
    ) -> Self {
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
    /// use domain::form::models::{Visibility, WebhookUrl};
    ///
    /// let administrator = User {
    ///     name: "administrator".to_string(),
    ///     id: Uuid::new_v4(),
    ///     role: Role::Administrator,
    /// };
    ///
    /// let standard_user = User {
    ///     name: "standard_user".to_string(),
    ///     id: Uuid::new_v4(),
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
    ///     FormTitle, Visibility, WebhookUrl
    /// };
    ///
    /// let administrator = User {
    ///     name: "administrator".to_string(),
    ///     id: Uuid::new_v4(),
    ///     role: Role::Administrator,
    /// };
    ///
    /// let standard_user = User {
    ///     name: "standard_user".to_string(),
    ///     id: Uuid::new_v4(),
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
    ///         AnswerVisibility::PRIVATE
    ///     ),
    ///     sample_questions(),
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
    ///         AnswerVisibility::PRIVATE
    ///     ),
    ///     sample_questions(),
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
    /// use domain::form::models::{FormDescription, FormTitle};
    ///
    /// let administrator = User {
    ///     name: "administrator".to_string(),
    ///     id: Uuid::new_v4(),
    ///     role: Role::Administrator,
    /// };
    ///
    /// let standard_user = User {
    ///     name: "standard_user".to_string(),
    ///     id: Uuid::new_v4(),
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
    /// use domain::form::models::{FormDescription, FormTitle};
    ///
    /// let administrator = User {
    ///     name: "administrator".to_string(),
    ///     id: Uuid::new_v4(),
    ///     role: Role::Administrator,
    /// };
    ///
    /// let standard_user = User {
    ///     name: "standard_user".to_string(),
    ///     id: Uuid::new_v4(),
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
    /// );
    ///
    /// assert!(!form.can_delete(&administrator));
    /// assert!(!form.can_delete(&standard_user));
    fn can_delete(&self, _actor: &User) -> bool {
        false
    }
}

pub type FormLabelId = types::Id<FormLabel>;

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
    ///     id: Uuid::new_v4(),
    ///     role: Role::Administrator,
    /// };
    ///
    /// let standard_user = User {
    ///     name: "standard_user".to_string(),
    ///     id: Uuid::new_v4(),
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
    ///     id: Uuid::new_v4(),
    ///     role: Role::Administrator,
    /// };
    ///
    /// let standard_user = User {
    ///     name: "standard_user".to_string(),
    ///     id: Uuid::new_v4(),
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
    ///     id: Uuid::new_v4(),
    ///     role: Role::Administrator,
    /// };
    ///
    /// let standard_user = User {
    ///     name: "standard_user".to_string(),
    ///     id: Uuid::new_v4(),
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
    ///     id: Uuid::new_v4(),
    ///     role: Role::Administrator,
    /// };
    ///
    /// let standard_user = User {
    ///     name: "standard_user".to_string(),
    ///     id: Uuid::new_v4(),
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
    use crate::user::models::Role;
    use types::non_empty_vec::NonEmptyVec;
    use uuid::Uuid;

    fn sample_questions() -> QuestionSet {
        QuestionSet::try_new(
            NonEmptyVec::try_new(vec![
                Question::new_text(
                    "body".to_string().try_into().unwrap(),
                    0,
                    "Body".to_string().try_into().unwrap(),
                    None,
                    true,
                )
                .unwrap(),
            ])
            .unwrap(),
        )
        .unwrap()
    }

    fn sample_form() -> ActiveForm {
        ActiveForm::from_raw_parts(
            FormId::from(Uuid::new_v4()),
            FormTitle::new("Form".to_string().try_into().unwrap()),
            FormDescription::new("description".to_string()),
            FormMeta::new(),
            FormSettings::new(),
            sample_questions(),
        )
    }

    fn admin_user() -> User {
        User {
            name: "admin".to_string(),
            id: Uuid::new_v4(),
            role: Role::Administrator,
        }
    }

    fn standard_user() -> User {
        User {
            name: "standard_user".to_string(),
            id: Uuid::new_v4(),
            role: Role::StandardUser,
        }
    }

    #[test]
    fn active_form_can_archive_for_administrator() {
        let form = sample_form();
        let actor = admin_user();
        let archived_at = Utc::now();

        let archived = form.clone().archive(archived_at, actor.clone());

        assert_eq!(archived.form(), &form);
        assert_eq!(archived.archived_at(), &archived_at);
        assert_eq!(archived.archived_by(), &actor);
    }

    #[test]
    fn active_form_archive_keeps_archived_by_as_given_actor() {
        let form = sample_form();
        let actor = standard_user();
        let archived_at = Utc::now();

        let archived = form.archive(archived_at, actor.clone());

        assert_eq!(archived.archived_at(), &archived_at);
        assert_eq!(archived.archived_by(), &actor);
    }

    #[test]
    fn archived_form_can_restore_to_active_form() {
        let form = sample_form();
        let archived =
            unsafe { ArchivedForm::from_raw_parts(form.clone(), Utc::now(), admin_user()) };

        let restored = archived.unarchive();

        assert_eq!(restored, form);
    }
}
