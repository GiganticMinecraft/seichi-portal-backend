use chrono::{DateTime, Utc};
#[cfg(test)]
use common::test_utils::arbitrary_opt_date_time;
use derive_getters::Getters;
use deriving_via::DerivingVia;
use errors::domain::DomainError;
#[cfg(test)]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};
use types::non_empty_string::NonEmptyString;

use crate::{
    form::answer::{AnswerAuthor, AnswerEntry},
    user::models::{Actor, Role, User},
};

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
pub struct AnswerAcceptancePeriod {
    #[cfg_attr(test, proptest(strategy = "arbitrary_opt_date_time()"))]
    #[serde(default)]
    start_at: Option<DateTime<Utc>>,
    #[cfg_attr(test, proptest(strategy = "arbitrary_opt_date_time()"))]
    #[serde(default)]
    end_at: Option<DateTime<Utc>>,
}

impl AnswerAcceptancePeriod {
    pub fn try_new(
        start_at: Option<DateTime<Utc>>,
        end_at: Option<DateTime<Utc>>,
    ) -> Result<Self, DomainError> {
        match (start_at, end_at) {
            (Some(start_at), Some(end_at)) if start_at > end_at => {
                Err(DomainError::InvalidAnswerAcceptancePeriod)
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
/// [`AnswerEntry`] の閲覧可否 ([`Self::can_read_entry`]) や新規受理 ([`Self::can_accept_answer`])
/// を判断します。この値オブジェクトは [`crate::form::models::ActiveForm`] が所有します。
#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Getters, Clone, Default, Debug, PartialEq)]
pub struct AnswerSettings {
    default_answer_title: DefaultAnswerTitle,
    visibility: AnswerVisibility,
    acceptance_period: AnswerAcceptancePeriod,
    allow_temporary_answers: bool,
}

impl AnswerSettings {
    pub fn new(
        default_answer_title: DefaultAnswerTitle,
        visibility: AnswerVisibility,
        acceptance_period: AnswerAcceptancePeriod,
        allow_temporary_answers: bool,
    ) -> Self {
        Self {
            default_answer_title,
            visibility,
            acceptance_period,
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

    pub fn change_acceptance_period(self, acceptance_period: AnswerAcceptancePeriod) -> Self {
        Self {
            acceptance_period,
            ..self
        }
    }

    pub fn change_allow_temporary_answers(self, allow_temporary_answers: bool) -> Self {
        Self {
            allow_temporary_answers,
            ..self
        }
    }

    /// `actor` が `author` として回答を作成してよいかを、受付期間と一時回答の可否から判定します。
    pub(crate) fn can_accept_answer(&self, author: &AnswerAuthor, actor: &Actor) -> bool {
        let is_within_period = self.acceptance_period.is_within_period(Utc::now());

        match (author, actor) {
            (AnswerAuthor::AuthenticatedUser(user_id), Actor::User(User::ActiveUser(user))) => {
                *user_id == *user.id() && (is_within_period || user.role() == &Role::Administrator)
            }
            (AnswerAuthor::TemporaryUser(_), Actor::User(User::TemporaryUser(_))) => {
                self.allow_temporary_answers && is_within_period
            }
            _ => false,
        }
    }

    /// `actor` が `entry` を閲覧できるかどうかを、回答の公開範囲をもとに判断します。
    pub fn can_read_entry(&self, entry: &AnswerEntry, actor: &Actor) -> bool {
        match actor {
            Actor::User(User::ActiveUser(user)) => {
                entry.author().authenticated_user_id() == Some(*user.id())
                    || self.visibility == AnswerVisibility::PUBLIC
                    || user.role() == &Role::Administrator
            }
            Actor::System => true,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        form::{
            answer::{AnswerTitle, PostedAnswerContents},
            models::FormId,
        },
        user::models::{ActiveUser, TemporaryUser, UserId},
    };
    use chrono::Duration;
    use uuid::Uuid;

    fn answer_settings(
        allow_temporary_answers: bool,
        acceptance_period: AnswerAcceptancePeriod,
    ) -> AnswerSettings {
        AnswerSettings::new(
            DefaultAnswerTitle::new(None),
            AnswerVisibility::PRIVATE,
            acceptance_period,
            allow_temporary_answers,
        )
    }

    fn active_user(role: Role) -> ActiveUser {
        ActiveUser::new("user".to_string(), UserId::from(Uuid::new_v4()), role)
    }

    fn answer_entry(author: AnswerAuthor) -> AnswerEntry {
        AnswerEntry::new(
            FormId::new(),
            author,
            AnswerTitle::new(None),
            PostedAnswerContents::try_new(&[], Vec::new()).unwrap(),
        )
    }

    #[test]
    fn temporary_answer_creation_requires_allow_flag() {
        let settings = answer_settings(false, AnswerAcceptancePeriod::try_new(None, None).unwrap());
        let author = AnswerAuthor::TemporaryUser(TemporaryUser::new(
            "guest".to_string(),
            "contact".to_string(),
        ));
        let actor = Actor::from(TemporaryUser::new(
            "guest".to_string(),
            "contact".to_string(),
        ));

        assert!(!settings.can_accept_answer(&author, &actor));
    }

    #[test]
    fn temporary_answer_creation_succeeds_when_allowed_and_within_period() {
        let settings = answer_settings(true, AnswerAcceptancePeriod::try_new(None, None).unwrap());
        let author = AnswerAuthor::TemporaryUser(TemporaryUser::new(
            "guest".to_string(),
            "contact".to_string(),
        ));
        let actor = Actor::from(TemporaryUser::new(
            "guest".to_string(),
            "contact".to_string(),
        ));

        assert!(settings.can_accept_answer(&author, &actor));
    }

    #[test]
    fn temporary_answer_creation_respects_acceptance_period() {
        let settings = answer_settings(
            true,
            AnswerAcceptancePeriod::try_new(
                Some(Utc::now() - Duration::days(2)),
                Some(Utc::now() - Duration::days(1)),
            )
            .unwrap(),
        );
        let author = AnswerAuthor::TemporaryUser(TemporaryUser::new(
            "guest".to_string(),
            "contact".to_string(),
        ));
        let actor = Actor::from(TemporaryUser::new(
            "guest".to_string(),
            "contact".to_string(),
        ));

        assert!(!settings.can_accept_answer(&author, &actor));
    }

    #[test]
    fn private_entry_is_readable_by_its_author() {
        let author = active_user(Role::StandardUser);
        let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
        let settings = answer_settings(false, AnswerAcceptancePeriod::try_new(None, None).unwrap());

        assert!(settings.can_read_entry(&entry, &Actor::from(author)));
    }

    #[test]
    fn private_entry_is_not_readable_by_other_standard_user() {
        let author = active_user(Role::StandardUser);
        let other = active_user(Role::StandardUser);
        let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
        let settings = answer_settings(false, AnswerAcceptancePeriod::try_new(None, None).unwrap());

        assert!(!settings.can_read_entry(&entry, &Actor::from(other)));
    }

    #[test]
    fn private_entry_is_readable_by_administrator() {
        let author = active_user(Role::StandardUser);
        let administrator = active_user(Role::Administrator);
        let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
        let settings = answer_settings(false, AnswerAcceptancePeriod::try_new(None, None).unwrap());

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
            AnswerAcceptancePeriod::try_new(None, None).unwrap(),
            false,
        );

        assert!(settings.can_read_entry(&entry, &Actor::from(other)));
    }
}
