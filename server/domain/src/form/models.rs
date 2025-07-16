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

use crate::{
    form::answer::settings::models::{
        AnswerSettings, AnswerVisibility, DefaultAnswerTitle, ResponsePeriod,
    },
    types::authorization_guard::AuthorizationGuardDefinitions,
    user::models::{Role::Administrator, User},
};

pub type FormId = types::Id<Form>;

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
#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
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
#[derive(Serialize, Deserialize, Getters, Debug, PartialEq)]
pub struct Form {
    #[serde(default)]
    id: FormId,
    title: FormTitle,
    #[serde(default)]
    description: FormDescription,
    #[serde(default)]
    metadata: FormMeta,
    #[serde(default)]
    settings: FormSettings,
}

impl Form {
    pub fn new(title: FormTitle, description: FormDescription) -> Self {
        Self {
            id: FormId::new(),
            title,
            description,
            metadata: FormMeta::new(),
            settings: FormSettings::new(),
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

    pub fn from_raw_parts(
        id: FormId,
        title: FormTitle,
        description: FormDescription,
        metadata: FormMeta,
        settings: FormSettings,
    ) -> Self {
        Self {
            id,
            title,
            description,
            metadata,
            settings,
        }
    }
}

impl AuthorizationGuardDefinitions for Form {
    /// [`Form`] の作成権限があるかどうかを判定します。
    ///
    /// 作成権限は以下の条件を満たしている場合に与えられます。
    /// - [`actor`] が [`Administrator`] である場合
    ///
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::models::{Form, FormSettings},
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
    /// let form = Form::new(
    ///     FormTitle::new("テストフォーム".to_string().try_into().unwrap()),
    ///     FormDescription::new("")
    /// );
    ///
    /// assert!(form.can_create(&administrator));
    /// assert!(!form.can_create(&standard_user));
    fn can_create(&self, actor: &User) -> bool {
        actor.role == Administrator
    }

    /// [`Form`] の読み取り権限があるかどうかを判定します。
    ///
    /// 読み取り権限は以下の条件のどちらかを満たしている場合に与えられます。
    /// - [`actor`] が [`Administrator`] である場合
    /// - [`Form`] が全体公開されている場合
    ///
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::models::{Form, FormSettings},
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
    /// let private_form = Form::from_raw_parts(
    ///     FormId::new(),
    ///     FormTitle::new("非公開フォーム".to_string().try_into().unwrap()),
    ///     FormDescription::new(""),
    ///     FormMeta::new(),
    ///     FormSettings::from_raw_parts(
    ///         ResponsePeriod::try_new(None, None).unwrap(),
    ///         WebhookUrl::try_new(None).unwrap(),
    ///         DefaultAnswerTitle::new(None),
    ///         Visibility::PRIVATE,
    ///         AnswerVisibility::PRIVATE
    ///     )
    /// );
    ///
    ///  let public_form = Form::from_raw_parts(
    ///     FormId::new(),
    ///     FormTitle::new("公開フォーム".to_string().try_into().unwrap()),
    ///     FormDescription::new(""),
    ///     FormMeta::new(),
    ///     FormSettings::from_raw_parts(
    ///         ResponsePeriod::try_new(None, None).unwrap(),
    ///         WebhookUrl::try_new(None).unwrap(),
    ///         DefaultAnswerTitle::new(None),
    ///         Visibility::PUBLIC,
    ///         AnswerVisibility::PRIVATE
    ///     )
    /// );
    ///
    /// assert!(private_form.can_read(&administrator));
    /// assert!(!private_form.can_read(&standard_user));
    /// assert!(public_form.can_read(&administrator));
    /// assert!(public_form.can_read(&standard_user));
    fn can_read(&self, actor: &User) -> bool {
        self.settings.visibility == Visibility::PUBLIC || actor.role == Administrator
    }

    /// [`Form`] の更新権限があるかどうかを判定します。
    ///
    /// 更新権限は以下の条件を満たしている場合に与えられます。
    /// - [`actor`] が [`Administrator`] である場合
    ///
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::models::{Form, FormSettings},
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
    /// let form = Form::new(
    ///     FormTitle::new("テストフォーム".to_string().try_into().unwrap()),
    ///     FormDescription::new("")
    /// );
    ///
    /// assert!(form.can_update(&administrator));
    /// assert!(!form.can_update(&standard_user));
    fn can_update(&self, actor: &User) -> bool {
        actor.role == Administrator
    }

    /// [`Form`] の削除権限があるかどうかを判定します。
    ///
    /// 削除権限は以下の条件を満たしている場合に与えられます。
    /// - [`actor`] が [`Administrator`] である場合
    ///
    /// # Examples
    /// ```
    /// use domain::{
    ///     form::models::{Form, FormSettings},
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
    /// let form = Form::new(
    ///     FormTitle::new("テストフォーム".to_string().try_into().unwrap()),
    ///     FormDescription::new("")
    /// );
    ///
    /// assert!(form.can_delete(&administrator));
    /// assert!(!form.can_delete(&standard_user));
    fn can_delete(&self, actor: &User) -> bool {
        actor.role == Administrator
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
