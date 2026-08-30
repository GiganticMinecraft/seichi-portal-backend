use chrono::{DateTime, Utc};
use derive_getters::Getters;
use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};

use crate::{
    account::models::Role,
    auth::Actor,
    form::{
        answer::{
            AnswerAuthor, AnswerStatus, AnswerStatusHistoryEntry, AnswerTitle,
            AnswerTitleHistoryEntry, FormAnswerContent, PostedAnswerContents,
            RedmineImportedAnswerReference, RedmineUserSnapshot,
        },
        models::{ActiveForm, ArchivedForm, FormId},
    },
    types::authorization_guard::{
        Allowed, AuthorizationRole, BelongsTo, Create, GuardedBy, ParentGuarded, Read, Update,
    },
};

pub type AnswerId = types::Id<AnswerEntry>;

/// アーカイブ済みフォーム配下に保存されている回答の identity です。
///
/// 関連の閲覧認可に必要な、回答 ID・所属フォーム・公開状態だけを保持します。
#[derive(UnsafeFromRawParts, Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchivedAnswerEntry {
    id: AnswerId,
    form_id: FormId,
    publication: AnswerPublication,
}

impl ArchivedAnswerEntry {
    pub fn id(&self) -> &AnswerId {
        &self.id
    }

    pub fn form_id(&self) -> &FormId {
        &self.form_id
    }

    pub fn publication(&self) -> &AnswerPublication {
        &self.publication
    }
}

/// 個別の回答を第三者へ公開するかどうかを表します。
#[derive(
    Serialize, Deserialize, Debug, EnumString, Display, Copy, Clone, Default, PartialEq, Eq,
)]
pub enum AnswerPublication {
    #[default]
    PUBLIC,
    PRIVATE,
}

impl TryFrom<String> for AnswerPublication {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        use std::str::FromStr;
        Self::from_str(&value).map_err(Into::into)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnswerPagePosition {
    last_timestamp: DateTime<Utc>,
    last_answer_id: AnswerId,
}

impl AnswerPagePosition {
    pub fn new(last_timestamp: DateTime<Utc>, last_answer_id: AnswerId) -> Self {
        Self {
            last_timestamp,
            last_answer_id,
        }
    }

    pub fn last_timestamp(self) -> DateTime<Utc> {
        self.last_timestamp
    }

    pub fn last_answer_id(self) -> AnswerId {
        self.last_answer_id
    }

    /// この位置の後に続く回答かを、回答一覧の降順で判定します。
    pub fn is_followed_by(self, timestamp: DateTime<Utc>, answer_id: AnswerId) -> bool {
        timestamp < self.last_timestamp
            || (timestamp == self.last_timestamp && answer_id < self.last_answer_id)
    }
}

#[derive(Serialize, Deserialize, Getters, Clone, PartialEq, Debug)]
pub struct AnswerEntry {
    id: AnswerId,
    form_id: FormId,
    author: AnswerAuthor,
    timestamp: DateTime<Utc>,
    title: AnswerTitle,
    publication: AnswerPublication,
    status: AnswerStatus,
    contents: Vec<FormAnswerContent>,
    redmine_reference: Option<RedmineImportedAnswerReference>,
}

impl AnswerEntry {
    /// 永続層から回答を復元します。呼び出し元は DB 行が整合していることを保証します。
    ///
    /// # Safety
    ///
    /// 呼び出し元は、各値が回答のドメイン不変条件を満たすことを保証しなければなりません。
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn from_raw_parts(
        id: AnswerId,
        form_id: FormId,
        author: AnswerAuthor,
        timestamp: DateTime<Utc>,
        title: AnswerTitle,
        publication: AnswerPublication,
        contents: Vec<FormAnswerContent>,
    ) -> Self {
        unsafe {
            Self::from_raw_parts_with_status_and_redmine_reference(
                id,
                form_id,
                author,
                timestamp,
                title,
                publication,
                AnswerStatus::default(),
                contents,
                None,
            )
        }
    }

    /// 永続層から Redmine 参照を含む回答を復元します。
    ///
    /// # Safety
    ///
    /// 呼び出し元は、各値と Redmine 参照が回答のドメイン不変条件を満たすことを保証しなければなりません。
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn from_raw_parts_with_redmine_reference(
        id: AnswerId,
        form_id: FormId,
        author: AnswerAuthor,
        timestamp: DateTime<Utc>,
        title: AnswerTitle,
        publication: AnswerPublication,
        contents: Vec<FormAnswerContent>,
        redmine_reference: Option<RedmineImportedAnswerReference>,
    ) -> Self {
        unsafe {
            Self::from_raw_parts_with_status_and_redmine_reference(
                id,
                form_id,
                author,
                timestamp,
                title,
                publication,
                AnswerStatus::default(),
                contents,
                redmine_reference,
            )
        }
    }

    /// 永続層から回答を status 付きで復元します。
    ///
    /// # Safety
    ///
    /// 呼び出し元は、各値が回答のドメイン不変条件を満たすことを保証します。
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn from_raw_parts_with_status_and_redmine_reference(
        id: AnswerId,
        form_id: FormId,
        author: AnswerAuthor,
        timestamp: DateTime<Utc>,
        title: AnswerTitle,
        publication: AnswerPublication,
        status: AnswerStatus,
        contents: Vec<FormAnswerContent>,
        redmine_reference: Option<RedmineImportedAnswerReference>,
    ) -> Self {
        Self {
            id,
            form_id,
            author,
            timestamp,
            title,
            publication,
            status,
            contents,
            redmine_reference,
        }
    }

    /// [`AnswerEntry`] を新しく作成します。
    pub fn new(
        form_id: FormId,
        author: AnswerAuthor,
        title: AnswerTitle,
        contents: PostedAnswerContents,
    ) -> Self {
        Self {
            id: AnswerId::new(),
            form_id,
            author,
            timestamp: Utc::now(),
            title,
            publication: AnswerPublication::PUBLIC,
            status: AnswerStatus::default(),
            contents: contents.into_inner(),
            redmine_reference: None,
        }
    }

    /// Redmine の issue を Portal の回答として新しく表します。
    ///
    /// Imported 回答は通常の回答投稿とは異なり、Redmine の作成日時・状態・公開範囲と
    /// issue 参照を最初から保持します。呼び出し側が通常の回答投稿認可や通知を通らない
    /// ことを明確にするため、この入口は Redmine author と issue ID を引数に取ります。
    #[allow(clippy::too_many_arguments)]
    pub fn import_from_redmine(
        form_id: FormId,
        issue_id: crate::form::answer::RedmineIssueId,
        author: RedmineUserSnapshot,
        timestamp: DateTime<Utc>,
        title: AnswerTitle,
        publication: AnswerPublication,
        status: AnswerStatus,
        contents: PostedAnswerContents,
    ) -> Result<Self, DomainError> {
        if issue_id.into_inner() <= 0 {
            return Err(DomainError::InvalidEntity {
                message: "Redmine issue ID must be positive".to_string(),
            });
        }
        author.validate()?;

        let id = AnswerId::new();

        Ok(Self {
            id,
            form_id,
            author: AnswerAuthor::ImportedFromRedmine(author),
            timestamp,
            title,
            publication,
            status,
            contents: contents.into_inner(),
            redmine_reference: Some(RedmineImportedAnswerReference::new(id, issue_id)),
        })
    }

    pub fn with_title(self, title: AnswerTitle) -> Self {
        Self { title, ..self }
    }

    pub fn change_publication(self, publication: AnswerPublication) -> Self {
        Self {
            publication,
            ..self
        }
    }

    pub fn change_status(self, status: AnswerStatus) -> Self {
        Self { status, ..self }
    }

    pub fn transition_status(
        self,
        status: AnswerStatus,
    ) -> Option<(Self, AnswerStatus, AnswerStatus)> {
        if self.status == status {
            None
        } else {
            let previous = self.status;
            Some((self.change_status(status), previous, status))
        }
    }

    pub(crate) fn publication_allows_read(&self, actor: &Actor) -> bool {
        match self.publication {
            AnswerPublication::PUBLIC => true,
            AnswerPublication::PRIVATE => {
                matches!(
                    actor,
                    Actor::AccountUser(user)
                        if user.role() == &Role::Administrator
                            || self.author.authenticated_user_id() == Some(*user.id())
                ) || matches!(actor, Actor::System)
            }
        }
    }
}

impl Allowed<AnswerEntry, Read> {
    pub fn authorize_status_history_entry(
        &self,
        entry: AnswerStatusHistoryEntry,
    ) -> Result<Allowed<AnswerStatusHistoryEntry, Read>, DomainError> {
        self.authorize_read(entry)
    }

    pub fn authorize_title_history_entry(
        &self,
        entry: AnswerTitleHistoryEntry,
    ) -> Result<Allowed<AnswerTitleHistoryEntry, Read>, DomainError> {
        self.authorize_read(entry)
    }
}

impl AuthorizationRole for AnswerEntry {
    type Role = ParentGuarded<ActiveForm>;
}

impl BelongsTo<ActiveForm> for AnswerEntry {
    fn belongs_to(&self, parent: &ActiveForm) -> bool {
        self.form_id() == parent.id()
    }
}

impl GuardedBy<ActiveForm, Read> for AnswerEntry {
    fn is_allowed_for(&self, parent: &ActiveForm, actor: &Actor) -> bool {
        parent.answer_settings().can_read_entry(self, actor)
    }
}

impl GuardedBy<ActiveForm, Update> for AnswerEntry {
    fn is_allowed_for(&self, _parent: &ActiveForm, actor: &Actor) -> bool {
        matches!(actor, Actor::AccountUser(user) if user.role() == &Role::Administrator)
    }
}

impl GuardedBy<ActiveForm, Create> for AnswerEntry {
    fn is_allowed_for(&self, parent: &ActiveForm, actor: &Actor) -> bool {
        parent
            .answer_settings()
            .can_accept_answer(self.author(), actor)
    }
}

impl AuthorizationRole for ArchivedAnswerEntry {
    type Role = ParentGuarded<ArchivedForm>;
}

impl BelongsTo<ArchivedForm> for ArchivedAnswerEntry {
    fn belongs_to(&self, parent: &ArchivedForm) -> bool {
        self.form_id() == parent.form().id()
    }
}

impl GuardedBy<ArchivedForm, Read> for ArchivedAnswerEntry {
    fn is_allowed_for(&self, _parent: &ArchivedForm, actor: &Actor) -> bool {
        matches!(self.publication, AnswerPublication::PUBLIC)
            || matches!(actor, Actor::System)
            || matches!(actor, Actor::AccountUser(user) if user.role() == &Role::Administrator)
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;
    use uuid::Uuid;

    use crate::form::answer::TemporaryAnswerAuthor;

    use super::*;

    fn empty_answer() -> AnswerEntry {
        AnswerEntry::new(
            FormId::new(),
            AnswerAuthor::Temporary(TemporaryAnswerAuthor::new(
                "name".to_string(),
                "contact".to_string(),
            )),
            AnswerTitle::default(),
            PostedAnswerContents::try_new(&[], Vec::new()).unwrap(),
        )
    }

    #[test]
    fn new_answer_is_unaddressed() {
        assert_eq!(*empty_answer().status(), AnswerStatus::UNADDRESSED);
    }

    #[test]
    fn status_transition_allows_every_different_pair_and_skips_noop() {
        let statuses = [
            AnswerStatus::UNADDRESSED,
            AnswerStatus::IN_PROGRESS,
            AnswerStatus::COMPLETED,
        ];

        for from in statuses {
            for to in statuses {
                let result = empty_answer().change_status(from).transition_status(to);
                if from == to {
                    assert!(result.is_none());
                } else {
                    let (answer, previous, next) = result.unwrap();
                    assert_eq!(*answer.status(), to);
                    assert_eq!(previous, from);
                    assert_eq!(next, to);
                }
            }
        }
    }

    #[test]
    fn page_position_follows_timestamp_desc_then_answer_id_desc() {
        let timestamp = Utc.with_ymd_and_hms(2026, 8, 3, 12, 0, 0).unwrap();
        let position = AnswerPagePosition::new(timestamp, Uuid::from_u128(3).into());

        assert!(position.is_followed_by(timestamp, Uuid::from_u128(2).into()));
        assert!(position.is_followed_by(
            Utc.with_ymd_and_hms(2026, 8, 3, 11, 59, 59).unwrap(),
            Uuid::from_u128(99).into(),
        ));
        assert!(!position.is_followed_by(timestamp, Uuid::from_u128(3).into()));
        assert!(!position.is_followed_by(timestamp, Uuid::from_u128(4).into()));
        assert!(!position.is_followed_by(
            Utc.with_ymd_and_hms(2026, 8, 3, 12, 0, 1).unwrap(),
            Uuid::from_u128(1).into(),
        ));
    }
}
