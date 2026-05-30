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
        comment::models::{Comment, CommentContent},
        models::FormId,
    },
    types::authorization_guard::{
        Allowed, AuthorizationGuard, AuthorizationGuardDefinitions, AuthorizesRead, Create, Read,
    },
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

pub type AnswerEntrySetId = types::Id<AnswerEntrySet>;

#[derive(Clone, Debug, PartialEq)]
pub struct AnswerEntrySet {
    id: AnswerEntrySetId,
    form_id: FormId,
    default_answer_title: DefaultAnswerTitle,
    visibility: AnswerVisibility,
    response_period: ResponsePeriod,
    allow_temporary_answers: bool,
    entries: Vec<AnswerEntry>,
}

impl AnswerEntrySet {
    pub fn new(
        form_id: FormId,
        default_answer_title: DefaultAnswerTitle,
        visibility: AnswerVisibility,
        response_period: ResponsePeriod,
        allow_temporary_answers: bool,
    ) -> Self {
        Self {
            id: AnswerEntrySetId::new(),
            form_id,
            default_answer_title,
            visibility,
            response_period,
            allow_temporary_answers,
            entries: Vec::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_raw_parts(
        id: AnswerEntrySetId,
        form_id: FormId,
        default_answer_title: DefaultAnswerTitle,
        visibility: AnswerVisibility,
        response_period: ResponsePeriod,
        allow_temporary_answers: bool,
        entries: Vec<AnswerEntry>,
    ) -> Self {
        Self {
            id,
            form_id,
            default_answer_title,
            visibility,
            response_period,
            allow_temporary_answers,
            entries,
        }
    }

    pub fn id(&self) -> &AnswerEntrySetId {
        &self.id
    }

    pub fn form_id(&self) -> &FormId {
        &self.form_id
    }

    pub fn default_answer_title(&self) -> &DefaultAnswerTitle {
        &self.default_answer_title
    }

    pub fn visibility(&self) -> &AnswerVisibility {
        &self.visibility
    }

    pub fn response_period(&self) -> &ResponsePeriod {
        &self.response_period
    }

    pub fn allow_temporary_answers(&self) -> &bool {
        &self.allow_temporary_answers
    }

    pub fn has_entries(&self) -> bool {
        !self.entries.is_empty()
    }

    pub fn readable_entries(&self, actor: &Actor) -> Vec<&AnswerEntry> {
        match actor {
            Actor::User(User::ActiveUser(user)) if user.role() == &Administrator => {
                self.entries.iter().collect()
            }
            Actor::User(User::ActiveUser(user)) => {
                if self.visibility == AnswerVisibility::PUBLIC {
                    self.entries.iter().collect()
                } else {
                    self.entries
                        .iter()
                        .filter(|e| e.author().authenticated_user_id() == Some(*user.id()))
                        .collect()
                }
            }
            Actor::System => self.entries.iter().collect(),
            _ => Vec::new(),
        }
    }

    pub(crate) fn find_entry(&self, answer_id: AnswerId) -> Option<&AnswerEntry> {
        self.entries.iter().find(|e| *e.id() == answer_id)
    }

    pub fn change_entry_title(
        self,
        answer_id: AnswerId,
        actor: &Actor,
        title: AnswerTitle,
    ) -> Result<Self, DomainError> {
        let entry = self.find_entry(answer_id).ok_or(DomainError::NotFound)?;
        self.check(actor, entry)?;
        let entries = self
            .entries
            .into_iter()
            .map(|entry| {
                if *entry.id() == answer_id {
                    entry.with_title(title.clone())
                } else {
                    entry
                }
            })
            .collect();
        Ok(Self { entries, ..self })
    }

    pub fn entries_as_system(&self, actor: &Actor) -> Result<&[AnswerEntry], DomainError> {
        match actor {
            Actor::System => Ok(&self.entries),
            _ => Err(DomainError::Forbidden),
        }
    }

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

    fn can_read_entry(&self, entry: &AnswerEntry, actor: &Actor) -> bool {
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
}

impl Allowed<AnswerEntrySet, Read> {
    pub fn readable_entries(&self) -> Vec<Allowed<AnswerEntry, Read>> {
        self.value()
            .readable_entries(self.actor())
            .into_iter()
            .filter_map(|entry| self.authorize_read(entry.clone()).ok())
            .collect()
    }

    pub fn read_entry(
        &self,
        answer_id: AnswerId,
    ) -> Result<Allowed<AnswerEntry, Read>, DomainError> {
        let entry = self
            .value()
            .find_entry(answer_id)
            .ok_or(DomainError::NotFound)?
            .clone();
        self.authorize_read(entry)
    }

    pub fn read_entry_for_comment(
        &self,
        answer_id: AnswerId,
    ) -> Result<Allowed<AnswerEntry, Read>, DomainError> {
        let entry = self.read_entry(answer_id)?;
        match self.actor() {
            Actor::User(User::ActiveUser(_)) => Ok(entry),
            _ => Err(DomainError::Forbidden),
        }
    }

    /// 対象の [`AnswerEntry`] が `actor` から閲覧可能であることをゲートとして検証したうえで、
    /// 新しい [`Comment`] の作成ガードを生成します。
    ///
    /// [`Comment`] はこのファクトリ経由でのみ生成できるため、文脈ゲートを通らない
    /// コメントが作られることはありません。
    pub fn create_comment(
        &self,
        answer_id: AnswerId,
        content: CommentContent,
    ) -> Result<AuthorizationGuard<Comment, Create>, DomainError> {
        self.read_entry_for_comment(answer_id)?;

        let commented_by = match self.actor() {
            Actor::User(User::ActiveUser(user)) => *user.id(),
            _ => return Err(DomainError::Forbidden),
        };

        Ok(AuthorizationGuard::from(Comment::new(
            answer_id,
            content,
            commented_by,
        )))
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

fn is_administrator(actor: &Actor) -> bool {
    matches!(actor, Actor::User(User::ActiveUser(user)) if user.role() == &Administrator)
}

impl AuthorizationGuardDefinitions for AnswerEntrySet {
    fn can_create(&self, actor: &Actor) -> bool {
        is_administrator(actor)
    }

    fn can_read(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::System | Actor::User(_))
    }

    fn can_update(&self, actor: &Actor) -> bool {
        is_administrator(actor)
    }

    fn can_delete(&self, _actor: &Actor) -> bool {
        false
    }
}

impl AuthorizesRead<AnswerEntry> for AnswerEntrySet {
    fn check(&self, actor: &Actor, child: &AnswerEntry) -> Result<(), DomainError> {
        if self.find_entry(*child.id()).is_none() {
            return Err(DomainError::NotFound);
        }

        if self.can_read_entry(child, actor) {
            Ok(())
        } else {
            Err(DomainError::Forbidden)
        }
    }
}

impl AuthorizesRead<Comment> for AnswerEntry {
    fn check(&self, _actor: &Actor, child: &Comment) -> Result<(), DomainError> {
        if child.answer_id() == self.id() {
            Ok(())
        } else {
            Err(DomainError::Forbidden)
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;
    use uuid::Uuid;

    use crate::form::{answer::models::AnswerTitle, models::FormId};
    use crate::user::models::{ActiveUser, Actor, Role, UserId};

    use super::*;

    fn answer_entry_set(
        allow_temporary_answers: bool,
        response_period: ResponsePeriod,
    ) -> AnswerEntrySet {
        AnswerEntrySet::new(
            FormId::new(),
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

    fn answer_entry_set_with_visibility(
        visibility: AnswerVisibility,
        entries: Vec<AnswerEntry>,
    ) -> AnswerEntrySet {
        AnswerEntrySet::from_raw_parts(
            AnswerEntrySetId::new(),
            FormId::new(),
            DefaultAnswerTitle::new(None),
            visibility,
            ResponsePeriod::try_new(None, None).unwrap(),
            false,
            entries,
        )
    }

    fn empty_posted_answers() -> PostedAnswerContents {
        PostedAnswerContents::try_new(&[], vec![]).unwrap()
    }

    #[test]
    fn temporary_answer_creation_requires_allow_flag() {
        let set = answer_entry_set(false, ResponsePeriod::try_new(None, None).unwrap());
        let author = AnswerAuthor::TemporaryUser(crate::user::models::TemporaryUser::new(
            "guest".to_string(),
            "contact".to_string(),
        ));
        let actor = Actor::from(crate::user::models::TemporaryUser::new(
            "guest".to_string(),
            "contact".to_string(),
        ));

        assert!(
            set.try_accept_answer(
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
        let set = answer_entry_set(true, ResponsePeriod::try_new(None, None).unwrap());
        let author = AnswerAuthor::TemporaryUser(crate::user::models::TemporaryUser::new(
            "guest".to_string(),
            "contact".to_string(),
        ));
        let actor = Actor::from(crate::user::models::TemporaryUser::new(
            "guest".to_string(),
            "contact".to_string(),
        ));

        assert!(
            set.try_accept_answer(
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
        let set = answer_entry_set(
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
            set.try_accept_answer(
                author,
                &actor,
                AnswerTitle::new(None),
                empty_posted_answers()
            )
            .is_err()
        );
    }

    #[test]
    fn private_entry_can_be_read_by_its_author() {
        let author = active_user(Role::StandardUser);
        let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
        let answer_id = *entry.id();
        let set = answer_entry_set_with_visibility(AnswerVisibility::PRIVATE, vec![entry]);

        let set = AuthorizationGuard::<_, Read>::from(set)
            .try_read(Actor::from(author))
            .unwrap();
        assert!(set.read_entry(answer_id).is_ok());
    }

    #[test]
    fn private_entry_cannot_be_read_by_other_standard_user() {
        let author = active_user(Role::StandardUser);
        let other = active_user(Role::StandardUser);
        let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
        let answer_id = *entry.id();
        let set = answer_entry_set_with_visibility(AnswerVisibility::PRIVATE, vec![entry]);

        let set = AuthorizationGuard::<_, Read>::from(set)
            .try_read(Actor::from(other))
            .unwrap();
        assert!(matches!(
            set.read_entry(answer_id),
            Err(DomainError::Forbidden)
        ));
    }

    #[test]
    fn private_entry_can_be_read_by_administrator() {
        let author = active_user(Role::StandardUser);
        let administrator = active_user(Role::Administrator);
        let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
        let answer_id = *entry.id();
        let set = answer_entry_set_with_visibility(AnswerVisibility::PRIVATE, vec![entry]);

        let set = AuthorizationGuard::<_, Read>::from(set)
            .try_read(Actor::from(administrator))
            .unwrap();
        assert!(set.read_entry(answer_id).is_ok());
    }

    #[test]
    fn public_entry_can_be_read_by_other_standard_user() {
        let author = active_user(Role::StandardUser);
        let other = active_user(Role::StandardUser);
        let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
        let answer_id = *entry.id();
        let set = answer_entry_set_with_visibility(AnswerVisibility::PUBLIC, vec![entry]);

        let set = AuthorizationGuard::<_, Read>::from(set)
            .try_read(Actor::from(other))
            .unwrap();
        assert!(set.read_entry(answer_id).is_ok());
    }

    #[test]
    fn comment_read_is_authorized_by_readable_answer_entry() {
        let author = active_user(Role::StandardUser);
        let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
        let answer_id = *entry.id();
        let comment = Comment::new(
            answer_id,
            CommentContent::new("comment".to_string().try_into().unwrap()),
            *author.id(),
        );
        let set = answer_entry_set_with_visibility(AnswerVisibility::PRIVATE, vec![entry]);

        assert!(
            AuthorizationGuard::<_, Read>::from(comment.clone())
                .try_read(Actor::from(author.clone()))
                .is_err()
        );

        let entry = AuthorizationGuard::<_, Read>::from(set)
            .try_read(Actor::from(author))
            .unwrap()
            .read_entry(answer_id)
            .unwrap();

        assert!(entry.authorize_comment(comment).is_ok());
    }

    #[test]
    fn comment_read_delegation_rejects_answer_mismatch() {
        let author = active_user(Role::StandardUser);
        let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
        let entry_id = *entry.id();
        let other_entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
        let other_entry_id = *other_entry.id();
        let comment = Comment::new(
            other_entry_id,
            CommentContent::new("comment".to_string().try_into().unwrap()),
            *author.id(),
        );
        let set = answer_entry_set_with_visibility(AnswerVisibility::PRIVATE, vec![entry]);
        let entry = AuthorizationGuard::<_, Read>::from(set)
            .try_read(Actor::from(author))
            .unwrap()
            .read_entry(entry_id)
            .unwrap();

        assert!(matches!(
            entry.authorize_comment(comment),
            Err(DomainError::Forbidden)
        ));
    }
}
