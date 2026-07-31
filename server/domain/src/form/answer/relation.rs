use errors::domain::DomainError;
use uuid::Uuid;

use crate::{
    form::models::FormId,
    types::authorization_guard::{Allowed, Read},
};

use super::{AnswerEntry, AnswerId, ArchivedAnswerEntry};

/// 回答を一意に参照するための、フォーム ID と回答 ID の組です。
///
/// 回答はアーカイブ時に別テーブルへ移動しますが、これらの UUID は変わらないため、
/// 関連の保存先ではテーブル名や外部キーではなくこの参照を使います。
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AnswerReference {
    form_id: FormId,
    answer_id: AnswerId,
}

impl AnswerReference {
    pub fn new(form_id: FormId, answer_id: AnswerId) -> Self {
        Self { form_id, answer_id }
    }

    pub fn form_id(self) -> FormId {
        self.form_id
    }

    pub fn answer_id(self) -> AnswerId {
        self.answer_id
    }

    /// DB の正規化規則と同じ順序で比較するキーを返します。
    pub(crate) fn ordering_key(self) -> (Uuid, Uuid) {
        (self.form_id.into_inner(), self.answer_id.into_inner())
    }
}

/// 回答関連の端点として必要な identity を提供します。
pub trait AnswerRelationEndpoint {
    fn answer_reference(&self) -> AnswerReference;
}

impl AnswerRelationEndpoint for AnswerEntry {
    fn answer_reference(&self) -> AnswerReference {
        AnswerReference::new(*self.form_id(), *self.id())
    }
}

impl AnswerRelationEndpoint for ArchivedAnswerEntry {
    fn answer_reference(&self) -> AnswerReference {
        AnswerReference::new(*self.form_id(), *self.id())
    }
}

/// 二つの回答間の直接的な、対称な関連です。
///
/// 端点は生成時に決定的な順序へ正規化されます。したがって A-B と B-A は同じ値であり、
/// この型から別の関連を推移的に導く API は提供しません。
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AnswerRelation {
    first: AnswerReference,
    second: AnswerReference,
}

impl AnswerRelation {
    pub fn new(first: AnswerReference, second: AnswerReference) -> Result<Self, DomainError> {
        if first == second {
            return Err(DomainError::InvalidEntity {
                message: "an answer cannot be related to itself".to_string(),
            });
        }

        let (first, second) = if first.ordering_key() < second.ordering_key() {
            (first, second)
        } else {
            (second, first)
        };

        Ok(Self { first, second })
    }

    pub fn first(self) -> AnswerReference {
        self.first
    }

    pub fn second(self) -> AnswerReference {
        self.second
    }

    pub fn endpoints(self) -> [AnswerReference; 2] {
        [self.first, self.second]
    }

    pub fn other_endpoint(self, endpoint: AnswerReference) -> Option<AnswerReference> {
        match self.endpoints() {
            [first, second] if first == endpoint => Some(second),
            [first, second] if second == endpoint => Some(first),
            _ => None,
        }
    }
}

impl Allowed<AnswerRelation, Read> {
    /// 回答 identity だけを使って、認可済み関係の反対側を取り出します。
    ///
    /// 呼び出し側は同じ Repository 呼び出しで認可された source identity を渡します。
    pub fn opposite_endpoint_for(
        &self,
        source: AnswerReference,
    ) -> Result<AnswerReference, DomainError> {
        self.value()
            .other_endpoint(source)
            .ok_or_else(|| DomainError::InvalidEntity {
                message: "source answer is not an endpoint of the relation".to_string(),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn reference(form_id: u128, answer_id: u128) -> AnswerReference {
        AnswerReference::new(
            Uuid::from_u128(form_id).into(),
            Uuid::from_u128(answer_id).into(),
        )
    }

    #[test]
    fn normalizes_endpoints_and_rejects_self_relations() {
        let first = reference(2, 1);
        let second = reference(1, 2);
        let relation = AnswerRelation::new(first, second).unwrap();

        assert_eq!(relation.endpoints(), [second, first]);
        assert_eq!(relation.other_endpoint(second), Some(first));
        assert!(AnswerRelation::new(first, first).is_err());
    }

    #[test]
    fn equality_is_symmetric_but_does_not_imply_transitivity() {
        let a = reference(1, 1);
        let b = reference(1, 2);
        let c = reference(1, 3);

        assert_eq!(AnswerRelation::new(a, b), AnswerRelation::new(b, a));
        assert_ne!(AnswerRelation::new(a, b), AnswerRelation::new(a, c));
    }

    #[test]
    fn allows_answers_from_different_forms() {
        let first_form_answer = reference(1, 1);
        let second_form_answer = reference(2, 1);

        let relation = AnswerRelation::new(first_form_answer, second_form_answer).unwrap();

        assert_eq!(
            relation.other_endpoint(first_form_answer),
            Some(second_form_answer)
        );
        assert_eq!(
            relation.other_endpoint(second_form_answer),
            Some(first_form_answer)
        );
    }
}
