use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::models::FormId,
    search::models::{
        AnswerLabelSearchHit, AnswerSearchHit, CommentSearchHit, FormLabelSearchHit, FormSearchHit,
        NumberOfRecordsPerAggregate, SearchableFieldsWithOperation, UserSearchHit,
    },
};

#[automock]
#[async_trait]
/// 全文検索エンジンへのリポジトリを操作をまとめた抽象
///
/// このプロジェクトでは、リポジトリ関数を定義するときに `AuthorizationGuard` または `Allowed` を受け取り、返すことでドメインモデルの認可状態を制御しているが、
/// 全文検索エンジンからの検索結果から認可に必要な情報を取得することは難しいので、このリポジトリでは「どの集約に検索がヒットしたか」という情報だけを提供する。
/// 実際にその集約を取得 / 操作する場合は、このリポジトリの返り値をもとに通常のリポジトリから集約を取得し、その際に認可ガードを適用する形になる。
pub trait SearchRepository: Send + Sync + 'static {
    async fn search_users(&self, query: &str) -> Result<Vec<UserSearchHit>, Error>;
    async fn search_forms(&self, query: &str) -> Result<Vec<FormSearchHit>, Error>;
    async fn search_labels_for_forms(&self, query: &str) -> Result<Vec<FormLabelSearchHit>, Error>;
    async fn search_labels_for_answers(
        &self,
        query: &str,
    ) -> Result<Vec<AnswerLabelSearchHit>, Error>;
    async fn search_answers(
        &self,
        query: &str,
        form_id: Option<FormId>,
    ) -> Result<Vec<AnswerSearchHit>, Error>;
    async fn search_comments(&self, query: &str) -> Result<Vec<CommentSearchHit>, Error>;
    async fn sync_search_engine(&self, data: &[SearchableFieldsWithOperation])
    -> Result<(), Error>;
    async fn fetch_search_engine_stats(&self) -> Result<NumberOfRecordsPerAggregate, Error>;
    /// 検索インデックスを初期化し、既存の回答文書に再投影が必要かを返す。
    async fn initialize_search_engine(&self) -> Result<bool, Error>;
}
