use domain_derive::UnsafeFromRawParts;
use errors::domain::DomainError;

use crate::{
    form::{
        answer::models::{AnswerEntry, AnswerId},
        comment::models::{Comment, CommentContent},
        models::FormId,
    },
    types::authorization_guard::{
        Allowed, AuthorizationRole, Authorizes, Create, Delete, ParentGuarded, Read, Update,
    },
    user::models::{Actor, Role, User},
};

/// あるフォームに紐づく回答 ([`AnswerEntry`]) の集合です。
///
/// この集約は「どの回答がこのフォームに属するか」という**構造**のみを担い、
/// 回答にまつわるポリシー（公開範囲・受付期間など）は持ちません。ポリシーは
/// [`ActiveForm`] が保持する [`crate::form::models::AnswerSettings`] が担当します。
///
/// 通常のリポジトリ取得では認可済みの [`ActiveForm`] から
/// [`crate::types::authorization_guard::Allowed`] を導出し、フォームとの所属検証や
/// 個々の回答の閲覧可否は [`ActiveForm`] のガードを起点とした連鎖で行われます。
///
/// [`ActiveForm`]: crate::form::models::ActiveForm
#[derive(UnsafeFromRawParts, Clone, Debug, PartialEq)]
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

impl AuthorizationRole for AnswerEntrySet {
    type Role = ParentGuarded;
}

impl Authorizes<Comment, Read> for AnswerEntry {
    fn check(&self, _actor: &Actor, child: &Comment) -> bool {
        child.answer_id() == self.id()
    }
}

impl Authorizes<Comment, Create> for AnswerEntry {
    fn check(&self, actor: &Actor, child: &Comment) -> bool {
        child.answer_id() == self.id()
            && matches!(actor, Actor::User(User::ActiveUser(user)) if user.id() == child.commented_by())
    }
}

impl Authorizes<Comment, Update> for AnswerEntry {
    fn check(&self, actor: &Actor, child: &Comment) -> bool {
        child.answer_id() == self.id()
            && matches!(actor, Actor::User(User::ActiveUser(user)) if user.id() == child.commented_by())
    }
}

impl Authorizes<Comment, Delete> for AnswerEntry {
    fn check(&self, actor: &Actor, child: &Comment) -> bool {
        child.answer_id() == self.id()
            && matches!(
                actor,
                Actor::User(User::ActiveUser(user))
                    if user.id() == child.commented_by() || user.role() == &Role::Administrator
            )
    }
}

impl Allowed<AnswerEntry, Read> {
    pub fn authorize_comment(
        &self,
        comment: Comment,
    ) -> Result<Allowed<Comment, Read>, DomainError> {
        self.authorize_read(comment)
    }

    pub fn create_comment(
        &self,
        content: CommentContent,
    ) -> Result<Allowed<Comment, Create>, DomainError> {
        let commented_by = match self.actor() {
            Actor::User(User::ActiveUser(user)) => *user.id(),
            _ => return Err(DomainError::Forbidden),
        };

        let comment = Comment::new(*self.value().id(), content, commented_by);

        self.authorize_create(comment)
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use crate::{
        form::answer::models::{AnswerAuthor, AnswerTitle, PostedAnswerContents},
        user::models::{ActiveUser, Role, UserId},
    };

    use super::*;

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

    #[test]
    fn find_entry_locates_member() {
        let author = active_user(Role::StandardUser);
        let entry = answer_entry(AnswerAuthor::AuthenticatedUser(*author.id()));
        let answer_id = *entry.id();
        let set = unsafe { AnswerEntrySet::from_raw_parts(FormId::new(), vec![entry]) };

        assert!(set.find_entry(answer_id).is_some());
        assert!(set.find_entry(AnswerId::new()).is_none());
    }
}
