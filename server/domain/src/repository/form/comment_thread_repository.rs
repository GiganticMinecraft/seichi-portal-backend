use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use chrono::{DateTime, Utc};

use crate::{
    form::{
        answer::AnswerEntry,
        comment::{Comment, CommentHistoryEntry, CommentHistoryPagePosition, DeletedComment},
        comment_thread::CommentThread,
        models::ActiveForm,
    },
    pagination::{Page, PageRequest},
    types::authorization_guard::{Allowed, Create, Read, Update},
};

#[automock]
#[async_trait]
pub trait CommentThreadRepository: Send + Sync + 'static {
    /// 最新のフォーム設定と回答から、コメントをロードせずに Thread を組み立てます。
    async fn get_for_answer(
        &self,
        form: &Allowed<ActiveForm, Read>,
        answer: AnswerEntry,
    ) -> Result<Allowed<CommentThread, Read>, Error>;
    /// 最新のフォーム設定、回答、DB からロードしたコメントを使って Thread を組み立てます。
    async fn get_with_comments_for_answer(
        &self,
        form: &Allowed<ActiveForm, Read>,
        answer: AnswerEntry,
    ) -> Result<Allowed<CommentThread, Read>, Error>;
    async fn create(
        &self,
        form: &Allowed<ActiveForm, Read>,
        comment: Allowed<Comment, Create>,
    ) -> Result<(), Error>;
    async fn update(
        &self,
        form: &Allowed<ActiveForm, Read>,
        comment: Allowed<Comment, Update>,
        updated_at: DateTime<Utc>,
    ) -> Result<(), Error>;
    async fn delete(
        &self,
        form: &Allowed<ActiveForm, Read>,
        comment: Allowed<DeletedComment, Create>,
    ) -> Result<(), Error>;
    async fn history(
        &self,
        comment_thread: &Allowed<CommentThread, Read>,
        request: PageRequest<CommentHistoryPagePosition>,
    ) -> Result<Page<Allowed<CommentHistoryEntry, Read>, CommentHistoryPagePosition>, Error>;
    async fn size(&self) -> Result<u32, Error>;
}
