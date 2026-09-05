use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::{
        comment::DeletedComment,
        comment_attachment::{CommentAttachment, CommentAttachmentBatch, CommentAttachmentId},
        comment_thread::CommentThread,
    },
    types::authorization_guard::{Allowed, Create, Delete, Read},
};

/// Portal コメント添付のメタデータと本体を扱う境界です。
///
/// 添付の認可は `CommentThread` から派生した [`Allowed`] で表現します。これにより、
/// 読み取りは親スレッドの読み取り認可を継承し、作成・削除は添付側の管理者認可を
/// Repository の型境界でも要求できます。
#[automock]
#[async_trait]
pub trait CommentAttachmentRepository: Send + Sync + 'static {
    /// 指定した親スレッド群の添付メタデータを一括取得し、各親の読み取り認可付きで返します。
    async fn read_all<'a>(
        &self,
        comment_threads: &[&'a Allowed<CommentThread, Read>],
    ) -> Result<Vec<Allowed<CommentAttachment, Read>>, Error>;

    /// 添付 ID を指定してメタデータを取得します。
    async fn get(
        &self,
        comment_thread: &Allowed<CommentThread, Read>,
        attachment_id: CommentAttachmentId,
    ) -> Result<Option<Allowed<CommentAttachment, Read>>, Error>;

    /// 同じコメントへの添付メタデータを一括作成します。
    async fn create_many(
        &self,
        attachments: Allowed<CommentAttachmentBatch, Create>,
        contents: Vec<Vec<u8>>,
    ) -> Result<(), Error>;

    /// 単一の添付メタデータと、本体を削除します。
    async fn delete(&self, attachment: Allowed<CommentAttachment, Delete>) -> Result<(), Error>;

    /// コメント削除に伴い、指定コメントに属する添付メタデータと本体を一括削除します。
    ///
    /// コメント自体の削除が認可済みであることを要求します。これは明示的な添付削除
    /// (`delete`) とは異なる、コメント削除に伴う cascade cleanup です。
    async fn delete_for_comment(
        &self,
        target: &Allowed<DeletedComment, Create>,
    ) -> Result<(), Error>;

    /// 認可済み添付の本体を取得します。
    async fn download(
        &self,
        attachment: &Allowed<CommentAttachment, Read>,
    ) -> Result<Vec<u8>, Error>;
}
