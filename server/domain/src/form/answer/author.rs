use derive_getters::Getters;
use deriving_via::DerivingVia;
use domain_derive::UnsafeFromRawParts;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::account::models::UserId;

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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum AnswerAuthor {
    AuthenticatedUser(UserId),
    TemporaryAnswerAuthor(TemporaryAnswerAuthor),
}

impl AnswerAuthor {
    pub fn authenticated_user_id(&self) -> Option<UserId> {
        match self {
            Self::AuthenticatedUser(user_id) => Some(*user_id),
            Self::TemporaryAnswerAuthor(_) => None,
        }
    }

    pub fn temporary_user(&self) -> Option<&TemporaryAnswerAuthor> {
        match self {
            Self::AuthenticatedUser(_) => None,
            Self::TemporaryAnswerAuthor(user) => Some(user),
        }
    }
}
