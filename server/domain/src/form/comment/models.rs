use chrono::{DateTime, Utc};
use derive_getters::Getters;
use deriving_via::DerivingVia;
use serde::{Deserialize, Serialize};
use types::non_empty_string::NonEmptyString;

use crate::{
    form::answer::models::AnswerId,
    types::authorization_guard::AuthorizationGuardDefinitions,
    user::models::{Actor, Role::Administrator, User, UserId},
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

#[derive(Serialize, Deserialize, Getters, Clone, Debug, PartialEq)]
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

    /// [`Comment`] を永続化済みのフィールド値から復元します。
    ///
    /// # Safety
    /// 新規作成ではなく、データベースなど信頼できる永続化済みデータの復元にのみ使用してください。
    pub unsafe fn from_raw_parts(
        answer_id: AnswerId,
        comment_id: CommentId,
        content: CommentContent,
        timestamp: DateTime<Utc>,
        commented_by: UserId,
    ) -> Self {
        Self {
            answer_id,
            comment_id,
            content,
            timestamp,
            commented_by,
        }
    }
}

/// [`Comment`] 自身で完結する認可のみをここで定義します。
///
/// 「紐づく [`AnswerEntry`](crate::form::answer::models::AnswerEntry) が閲覧可能か」
/// という文脈依存の判定は AnswerEntry 側のゲートが担うため、ここには含めません。
impl AuthorizationGuardDefinitions for Comment {
    fn can_create(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(user)) if *user.id() == self.commented_by)
    }

    fn can_read(&self, _actor: &Actor) -> bool {
        false
    }

    fn can_update(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(user)) if *user.id() == self.commented_by)
    }

    fn can_delete(&self, actor: &Actor) -> bool {
        matches!(
            actor,
            Actor::User(User::ActiveUser(user))
                if *user.id() == self.commented_by || user.role() == &Administrator
        )
    }
}
