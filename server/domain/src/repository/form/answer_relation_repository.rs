use async_trait::async_trait;
use errors::Error;

use crate::{
    form::{
        answer::{AnswerEntry, AnswerId},
        models::FormId,
    },
    types::authorization_guard::{Allowed, Update},
};

/// 保存先での回答のライフサイクルです。関連先の本文はここでは保持しません。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelatedAnswerLifecycle {
    Active,
    Archived,
}

/// 関連先を API に参照として返すための read model です。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelatedAnswerReference {
    pub form_id: FormId,
    pub answer_id: AnswerId,
    pub lifecycle: RelatedAnswerLifecycle,
}

#[async_trait]
pub trait AnswerRelationRepository: Send + Sync + 'static {
    /// 関連の全置換が実行可能かを、副作用なしで確認します。
    /// 実行時にも競合を防ぐため [`Self::replace_for_answer`] が同じ検証を再実行します。
    async fn validate_replace_for_answer(
        &self,
        answer: &Allowed<AnswerEntry, Update>,
        related_answer_ids: &[AnswerId],
    ) -> Result<(), Error>;

    /// 指定回答から伸びる直接関連を、入力集合で全置換します。
    /// `answer` の更新認可は関連の作成・削除に必要な管理者認可の証明です。
    async fn replace_for_answer(
        &self,
        answer: Allowed<AnswerEntry, Update>,
        related_answer_ids: Vec<AnswerId>,
    ) -> Result<(), Error>;

    /// 回答メタデータと関連を同一の永続化単位で更新します。
    /// 関連指定を伴う PATCH が、並行アーカイブによる関係更新失敗だけを残さないための境界です。
    async fn update_answer_meta_and_replace(
        &self,
        answer: Allowed<AnswerEntry, Update>,
        related_answer_ids: Vec<AnswerId>,
    ) -> Result<(), Error>;

    /// 指定回答と直接つながる回答を取得します。呼び出し側は表示前に各参照を
    /// 個別の Read 認可で絞り込みます。
    async fn list_for_answer(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<RelatedAnswerReference>, Error>;
}
