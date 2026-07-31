use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::answer::{AnswerEntry, AnswerId, AnswerRelation, ArchivedAnswerEntry},
    types::authorization_guard::{Allowed, Read, Update},
};

/// 回答間の直接関係を保存・取得する境界です。
///
/// 関係の作成・解除に必要な回答は、親フォームと回答の更新認可を得た
/// [`Allowed`] でなければ渡せません。これにより usecase の事前チェックだけに依存せず、
/// Repository 境界でも管理者操作であることを表現します。
#[automock]
#[async_trait]
pub trait AnswerRelationRepository: Send + Sync + 'static {
    /// `source` に直接つながる関係を、DB の決定的な順序で返します。
    async fn list_for_answer(
        &self,
        source: &Allowed<AnswerEntry, Read>,
    ) -> Result<Vec<Allowed<AnswerRelation, Read>>, Error>;

    /// 認可済みアーカイブ回答を起点にした参照です。
    async fn list_for_archived_answer(
        &self,
        source: &Allowed<ArchivedAnswerEntry, Read>,
    ) -> Result<Vec<Allowed<AnswerRelation, Read>>, Error>;

    /// 関係を追加します。同じ正規化済み関係が存在する場合は成功として扱います。
    async fn add(
        &self,
        relation: AnswerRelation,
        source: &Allowed<AnswerEntry, Update>,
        target: &Allowed<AnswerEntry, Update>,
    ) -> Result<(), Error>;

    /// 関係を解除します。関係が存在しない場合は成功として扱います。
    async fn remove(
        &self,
        relation: AnswerRelation,
        source: &Allowed<AnswerEntry, Update>,
        target: &Allowed<AnswerEntry, Update>,
    ) -> Result<(), Error>;

    /// `source` と指定した回答 ID の間にある関係を探します。
    ///
    /// DELETE の URL は関連先フォーム ID を含まないため、関係が存在する場合に対象回答の
    /// 安定したフォーム ID を復元するために使います。関係がない場合は `None` です。
    async fn find_for_source_and_answer_id(
        &self,
        source: &Allowed<AnswerEntry, Update>,
        answer_id: AnswerId,
    ) -> Result<Option<Allowed<AnswerRelation, Read>>, Error>;
}
