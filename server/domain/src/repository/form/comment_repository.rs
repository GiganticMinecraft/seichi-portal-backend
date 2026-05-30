use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::{answer::models::AnswerId, comment::models::Comment},
    types::authorization_guard::{AuthorizationGuard, Create, Delete, Update},
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
    async fn find_by_answer_id(&self, answer_id: AnswerId) -> Result<Vec<Comment>, Error>;
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
    async fn get_all(&self) -> Result<Vec<Comment>, Error>;
    async fn size(&self) -> Result<u32, Error>;
}
