use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::{answer::models::AnswerEntry, comment::models::Comment},
    types::authorization_guard::{AuthorizationGuard, Create, Delete, Read, Update},
    user::models::Actor,
};

/// [`Comment`] を集約ルートとして永続化するためのリポジトリ。
///
/// [`Comment`] の生成は [`AnswerEntrySet::create_comment`](crate::form::answer_entry_set::models::AnswerEntrySet::create_comment)
/// 経由でのみ行えるため、`create` には常に文脈ゲートを通過済みの
/// [`AuthorizationGuard<Comment, Create>`] が渡される。
#[automock]
#[async_trait]
pub trait CommentRepository: Send + Sync + 'static {
    async fn create(
        &self,
        comment: AuthorizationGuard<Comment, Create>,
        actor: &Actor,
    ) -> Result<(), Error>;
    /// 閲覧可能であることが確認済みの [`AnswerEntry`] に紐づくコメントを取得する。
    ///
    /// コメントを読むには紐づく [`AnswerEntry`] が閲覧可能である必要があるため、
    /// その前提を引数で要求し、結果は [`Read`] ガードで保護して返す。
    async fn find_by_answer(
        &self,
        answer: &AnswerEntry,
    ) -> Result<Vec<AuthorizationGuard<Comment, Read>>, Error>;
    async fn update(
        &self,
        comment: AuthorizationGuard<Comment, Update>,
        actor: &Actor,
    ) -> Result<(), Error>;
    async fn delete(
        &self,
        comment: AuthorizationGuard<Comment, Delete>,
        actor: &Actor,
    ) -> Result<(), Error>;
    async fn get_all(&self) -> Result<Vec<AuthorizationGuard<Comment, Read>>, Error>;
    async fn size(&self) -> Result<u32, Error>;
}
