use chrono::{DateTime, Utc};
use derive_getters::Getters;
use deriving_via::DerivingVia;
use domain_derive::UnsafeFromRawParts;
use serde::{Deserialize, Serialize};
use types::non_empty_string::NonEmptyString;

use crate::{
    form::answer::models::AnswerId,
    types::authorization_guard::{AuthorizationRole, ParentGuarded},
    user::models::UserId,
};

pub type CommentId = types::Id<Comment>;

#[derive(DerivingVia, Debug, PartialEq)]
#[deriving(Clone, From, Into, IntoInner, Serialize, Deserialize)]
pub struct CommentContent(NonEmptyString);

impl CommentContent {
    pub fn new(content: NonEmptyString) -> Self {
        Self(content)
    }
}

#[derive(UnsafeFromRawParts, Serialize, Deserialize, Getters, Clone, Debug, PartialEq)]
pub struct Comment {
    answer_id: AnswerId,
    comment_id: CommentId,
    content: CommentContent,
    timestamp: DateTime<Utc>,
    commented_by: UserId,
}

impl Comment {
    /// [`Comment`] を新しく作成します。
    ///
    /// コメントの生成は必ず紐づく [`AnswerEntry`](crate::form::answer::models::AnswerEntry)
    /// の認可ゲートを通す必要があるため、この関数は crate 内
    /// (集約のファクトリ) からのみ呼び出せるようにしてあります。
    pub(crate) fn new(answer_id: AnswerId, content: CommentContent, commented_by: UserId) -> Self {
        Self {
            answer_id,
            comment_id: CommentId::new(),
            content,
            timestamp: Utc::now(),
            commented_by,
        }
    }

    pub fn with_updated_content(self, content: CommentContent) -> Self {
        Self { content, ..self }
    }
}

// [`Comment`] は自己ガード ([`crate::types::authorization_guard::AuthorizationGuardDefinitions`])
// を実装しない。コメントは親である
// [`AnswerEntry`](crate::form::answer::models::AnswerEntry) のガードを起点としてのみ
// 認可され、その条件 (閲覧・作成・更新・削除) は
// [`crate::form::answer::models`] の `Authorizes<Comment, _>` が担う。

impl AuthorizationRole for Comment {
    type Role = ParentGuarded;
}
