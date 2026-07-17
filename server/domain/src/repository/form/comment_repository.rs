use async_trait::async_trait;
use chrono::{DateTime, Utc};
use errors::Error;
use mockall::automock;

use crate::{
    form::{
        answer::AnswerEntry,
        comment::{Comment, CommentHistoryEntry, CommentHistoryPagePosition, DeletedComment},
    },
    pagination::{Page, PageRequest},
    types::authorization_guard::{Allowed, Create, Delete, Read, Update},
};

/// [`Comment`] を集約ルートとして永続化するためのリポジトリ。
///
/// [`Comment`] の生成は [`ActiveForm`](crate::form::models::ActiveForm) のガード経由
/// (`Allowed<ActiveForm, Read>::create_comment`) でのみ行えるため、`create` には作成操作の認可を通過した
/// [`Allowed<Comment, Create>`] が渡される。
#[automock]
#[async_trait]
pub trait CommentRepository: Send + Sync + 'static {
    async fn create(&self, comment: Allowed<Comment, Create>) -> Result<(), Error>;
    /// 閲覧可能であることが確認済みの [`AnswerEntry`] に紐づくコメントを取得する。
    ///
    /// コメントを読むには紐づく [`AnswerEntry`] が閲覧可能である必要があるため、
    /// 引数の [`Allowed<AnswerEntry, Read>`] から各コメントの
    /// [`Allowed<Comment, Read>`] を導出して返す。
    async fn find_by_answer(
        &self,
        answer: &Allowed<AnswerEntry, Read>,
    ) -> Result<Vec<Allowed<Comment, Read>>, Error>;
    async fn update(
        &self,
        comment: Allowed<Comment, Update>,
        updated_at: DateTime<Utc>,
    ) -> Result<(), Error>;
    async fn delete(&self, comment: Allowed<DeletedComment, Delete>) -> Result<(), Error>;
    async fn history(
        &self,
        answer: &Allowed<AnswerEntry, Read>,
        request: PageRequest<CommentHistoryPagePosition>,
    ) -> Result<Page<Allowed<CommentHistoryEntry, Read>, CommentHistoryPagePosition>, Error>;
    async fn size(&self) -> Result<u32, Error>;
}
