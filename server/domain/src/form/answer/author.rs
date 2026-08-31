use derive_getters::Getters;
use deriving_via::DerivingVia;
use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::account::models::UserId;

use super::entry::AnswerId;

#[derive(DerivingVia, Debug, PartialOrd, PartialEq, Eq, Hash, Clone, Copy)]
#[deriving(
    From,
    Into,
    IntoInner(via: Uuid),
    Display(via: Uuid),
    Serialize(via: Uuid),
    Deserialize(via: Uuid)
)]
pub struct TemporaryAnswerAuthorId(#[underlying] Uuid);

/// 一時回答が許可されたフォームで、ログインせずに回答した人の著者情報。
///
/// `TemporaryAnswerAuthor` は永続的な認証主体ではなく、回答作成時に入力された情報を
/// 回答の著者として保持するためのスナップショットである。`id` は通常の
/// `UserId` やログインセッションとは別の、回答著者を一時ユーザーとして識別する
/// ローカルな UUID として扱う。
///
/// `name` と `contact_text` は、管理者や回答閲覧者が回答者を識別し、必要に応じて
/// 連絡するための入力値である。権限判定上は回答の作成主体としてだけ使われ、
/// 登録済みアカウントと同じ閲覧・更新権限は持たない。
#[derive(UnsafeFromRawParts, Serialize, Deserialize, Getters, Debug, Clone, PartialEq, Eq)]
pub struct TemporaryAnswerAuthor {
    id: TemporaryAnswerAuthorId,
    name: String,
    contact_text: String,
}

impl TemporaryAnswerAuthor {
    pub fn new(name: String, contact_text: String) -> Self {
        Self {
            id: TemporaryAnswerAuthorId::from(Uuid::new_v4()),
            name,
            contact_text,
        }
    }
}

/// Redmine 側で回答を書き込んだ利用者の、移行時点の表示用スナップショットです。
///
/// Portal の `UserId` やロールは保持しません。Redmine の利用者が Portal の認証主体に
/// なることはなく、表示名は importer が補完した必須値として扱います。
#[derive(UnsafeFromRawParts, Serialize, Deserialize, Getters, Debug, Clone, PartialEq, Eq)]
pub struct RedmineUserSnapshot {
    redmine_user_id: Option<i64>,
    display_name: String,
}

impl RedmineUserSnapshot {
    pub fn new(redmine_user_id: Option<i64>, display_name: String) -> Self {
        Self {
            redmine_user_id,
            display_name,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), DomainError> {
        if self.redmine_user_id.is_some_and(|id| id <= 0) {
            return Err(DomainError::InvalidEntity {
                message: "Redmine user ID must be positive when present".to_string(),
            });
        }
        if self.display_name.trim().is_empty() {
            return Err(DomainError::InvalidEntity {
                message: "Redmine author display name must not be empty".to_string(),
            });
        }

        Ok(())
    }

    pub fn try_new(
        redmine_user_id: Option<i64>,
        display_name: String,
    ) -> Result<Self, DomainError> {
        let snapshot = Self::new(redmine_user_id, display_name);
        snapshot.validate()?;
        Ok(snapshot)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AnswerAuthor {
    AuthenticatedUser(UserId),
    Temporary(TemporaryAnswerAuthor),
    ImportedFromRedmine(RedmineUserSnapshot),
}

impl AnswerAuthor {
    pub fn authenticated_user_id(&self) -> Option<UserId> {
        match self {
            Self::AuthenticatedUser(user_id) => Some(*user_id),
            Self::Temporary(_) | Self::ImportedFromRedmine(_) => None,
        }
    }

    pub fn temporary_user(&self) -> Option<&TemporaryAnswerAuthor> {
        match self {
            Self::AuthenticatedUser(_) | Self::ImportedFromRedmine(_) => None,
            Self::Temporary(user) => Some(user),
        }
    }

    pub fn redmine_user(&self) -> Option<&RedmineUserSnapshot> {
        match self {
            Self::ImportedFromRedmine(user) => Some(user),
            Self::AuthenticatedUser(_) | Self::Temporary(_) => None,
        }
    }
}

/// Redmine の issue ID です。
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RedmineIssueId(i64);

impl RedmineIssueId {
    pub fn new(value: i64) -> Self {
        Self(value)
    }

    pub fn try_new(value: i64) -> Result<Self, DomainError> {
        if value <= 0 {
            return Err(DomainError::InvalidEntity {
                message: "Redmine issue ID must be positive".to_string(),
            });
        }

        Ok(Self::new(value))
    }

    pub fn into_inner(self) -> i64 {
        self.0
    }
}

impl From<i64> for RedmineIssueId {
    fn from(value: i64) -> Self {
        Self::new(value)
    }
}

impl From<RedmineIssueId> for i64 {
    fn from(value: RedmineIssueId) -> Self {
        value.into_inner()
    }
}

/// Redmine issue と Portal の回答を一対一に対応付ける参照です。
#[derive(Serialize, Deserialize, Getters, Debug, Clone, Copy, PartialEq, Eq)]
pub struct RedmineImportedAnswerReference {
    answer_id: AnswerId,
    issue_id: RedmineIssueId,
}

impl RedmineImportedAnswerReference {
    pub fn new(answer_id: AnswerId, issue_id: RedmineIssueId) -> Self {
        Self {
            answer_id,
            issue_id,
        }
    }
}
