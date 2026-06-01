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
    form::{
        answer::models::{AnswerAuthor, AnswerEntry, AnswerId, AnswerTitle, PostedAnswerContents},
        comment::models::Comment,
        models::FormId,
    },
    types::authorization_guard::{Allowed, Authorizes, Read},
    user::models::{Actor, Role::Administrator, User},
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
/// を判断します。この値オブジェクトは [`ActiveForm`] が所有し、回答の集合である
/// [`AnswerEntrySet`] は構造（所属）のみを担います。
///
/// [`ActiveForm`]: crate::form::models::ActiveForm
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
    pub fn try_accept_answer(
        &self,
        author: AnswerAuthor,
        actor: &Actor,
        title: AnswerTitle,
        posted_answers: PostedAnswerContents,
    ) -> Result<AnswerEntry, DomainError> {
        let is_within_period = self.response_period.is_within_period(Utc::now());

        let allowed = match (&author, actor) {
            (AnswerAuthor::AuthenticatedUser(user_id), Actor::User(User::ActiveUser(user))) => {
                *user_id == *user.id() && (is_within_period || user.role() == &Administrator)
            }
            (AnswerAuthor::TemporaryUser(_), Actor::User(User::TemporaryUser(_))) => {
                self.allow_temporary_answers && is_within_period
            }
            _ => false,
        };

        if !allowed {
            return Err(DomainError::Forbidden);
        }

        Ok(AnswerEntry::new(author, title, posted_answers))
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

/// あるフォームに紐づく回答 ([`AnswerEntry`]) の集合です。
///
/// この集約は「どの回答がこのフォームに属するか」という**構造**のみを担い、
/// 回答にまつわるポリシー（公開範囲・受付期間など）は持ちません。ポリシーは
/// [`ActiveForm`] が保持する [`AnswerSettings`] が担当します。
///
/// 通常のリポジトリ取得では認可済みの [`ActiveForm`] から
/// [`crate::types::authorization_guard::Allowed`] を導出し、フォームとの所属検証や
/// 個々の回答の閲覧可否は [`ActiveForm`] のガードを起点とした連鎖で行われます。
///
/// [`ActiveForm`]: crate::form::models::ActiveForm
#[derive(Clone, Debug, PartialEq)]
pub struct AnswerEntrySet {
    form_id: FormId,
    entries: Vec<AnswerEntry>,
}

impl AnswerEntrySet {
    pub fn new(form_id: FormId) -> Self {
        Self {
            form_id,
            entries: Vec::new(),
        }
    }

    pub fn from_raw_parts(form_id: FormId, entries: Vec<AnswerEntry>) -> Self {
        Self { form_id, entries }
    }

    pub fn form_id(&self) -> &FormId {
        &self.form_id
    }

    pub fn entries(&self) -> &[AnswerEntry] {
        &self.entries
    }

    pub fn has_entries(&self) -> bool {
        !self.entries.is_empty()
    }

    pub fn find_entry(&self, answer_id: AnswerId) -> Option<&AnswerEntry> {
        self.entries.iter().find(|e| *e.id() == answer_id)
    }
}

impl Authorizes<Comment, Read> for AnswerEntry {
    fn check(&self, _actor: &Actor, child: &Comment) -> Result<(), DomainError> {
        if child.answer_id() == self.id() {
            Ok(())
        } else {
            Err(DomainError::Forbidden)
        }
    }
}

impl Allowed<AnswerEntry, Read> {
    pub fn authorize_comment(
        &self,
        comment: Comment,
    ) -> Result<Allowed<Comment, Read>, DomainError> {
        self.authorize_read(comment)
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use uuid::Uuid;

    use crate::form::answer::models::AnswerTitle;
    use crate::user::models::{ActiveUser, Actor, Role, UserId};

    use super::*;

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

    fn active_user(role: Role) -> ActiveUser {
        ActiveUser::new("user".to_string(), UserId::from(Uuid::new_v4()), role)
    }

    fn answer_entry(author: AnswerAuthor) -> AnswerEntry {
        AnswerEntry::new(
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
    fn find_entry_locates_member() {
        let author = active_user(Role::StandardUser);
        let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
        let answer_id = *entry.id();
        let set = AnswerEntrySet::from_raw_parts(FormId::new(), vec![entry]);

        assert!(set.find_entry(answer_id).is_some());
        assert!(set.find_entry(AnswerId::new()).is_none());
    }
}
