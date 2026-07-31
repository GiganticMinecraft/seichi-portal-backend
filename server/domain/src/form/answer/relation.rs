use errors::domain::DomainError;
use uuid::Uuid;

use crate::{
    auth::Actor,
    form::models::FormId,
    types::authorization_guard::{
        Actions, Allowed, AuthorizationGuard, AuthorizationGuardDefinitions, AuthorizationRole,
        Read, SelfGuarded, Update,
    },
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

/// 両端点の閲覧認可を同じ利用者について確認済みである回答関連です。
///
/// `AnswerRelation` 自体にはフォーム設定や回答の公開状態が含まれないため、関連単体の
/// `AuthorizationGuard` では閲覧可否を判定できません。この型は Repository が両端点の
/// `Allowed` を確認した後に、その認可に使った `Actor` を保持した読み取り対象として
/// 生成されます。これにより、別の利用者へ読み取り証明を再利用できません。
#[derive(Clone, Debug, PartialEq)]
pub struct ReadableAnswerRelation {
    relation: AnswerRelation,
    actor: Actor,
}

impl ReadableAnswerRelation {
    fn new(relation: AnswerRelation, actor: Actor) -> Self {
        Self { relation, actor }
    }

    pub fn relation(&self) -> AnswerRelation {
        self.relation
    }

    pub fn into_relation(self) -> AnswerRelation {
        self.relation
    }

    pub fn opposite_endpoint_for(
        &self,
        source: AnswerReference,
    ) -> Result<AnswerReference, DomainError> {
        self.relation
            .other_endpoint(source)
            .ok_or_else(|| DomainError::InvalidEntity {
                message: "source answer is not an endpoint of the relation".to_string(),
            })
    }
}

impl AuthorizationRole for ReadableAnswerRelation {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for ReadableAnswerRelation {
    fn can_create(&self, _actor: &Actor) -> bool {
        false
    }

    fn can_read(&self, actor: &Actor) -> bool {
        &self.actor == actor
    }

    fn can_update(&self, _actor: &Actor) -> bool {
        false
    }

    fn can_delete(&self, _actor: &Actor) -> bool {
        false
    }
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

    pub fn connects<S, T>(&self, source: &S, target: &T) -> bool
    where
        S: AnswerRelationEndpoint,
        T: AnswerRelationEndpoint,
    {
        self.other_endpoint(source.answer_reference()) == Some(target.answer_reference())
    }

    fn authorize_read_with_proofs<S, T, SourceAction, TargetAction>(
        self,
        source: &Allowed<S, SourceAction>,
        target: &Allowed<T, TargetAction>,
    ) -> Result<Allowed<ReadableAnswerRelation, Read>, DomainError>
    where
        S: AnswerRelationEndpoint,
        T: AnswerRelationEndpoint,
        SourceAction: Actions,
        TargetAction: Actions,
    {
        if source.actor() != target.actor() {
            return Err(DomainError::InvalidEntity {
                message: "answer relation proofs must belong to the same actor".to_string(),
            });
        }

        if !self.connects(source.value(), target.value()) {
            return Err(DomainError::InvalidEntity {
                message: "answer relation endpoints do not match authorized answers".to_string(),
            });
        }

        AuthorizationGuard::<ReadableAnswerRelation, Read>::from(ReadableAnswerRelation::new(
            self,
            source.actor().clone(),
        ))
        .try_read(source.actor().clone())
    }

    /// source と target の回答 Read proof を合成し、関連自体の Read proof を作ります。
    pub fn authorize_read<S, T>(
        self,
        source: &Allowed<S, Read>,
        target: &Allowed<T, Read>,
    ) -> Result<Allowed<ReadableAnswerRelation, Read>, DomainError>
    where
        S: AnswerRelationEndpoint,
        T: AnswerRelationEndpoint,
    {
        self.authorize_read_with_proofs(source, target)
    }

    /// 更新認可済みの source と、対象の Read proof から関連の Read proof を作ります。
    pub fn authorize_read_from_update<T>(
        self,
        source: &Allowed<AnswerEntry, Update>,
        target: &Allowed<T, Read>,
    ) -> Result<Allowed<ReadableAnswerRelation, Read>, DomainError>
    where
        T: AnswerRelationEndpoint,
    {
        self.authorize_read_with_proofs(source, target)
    }

    /// 関連追加用に、source と target の更新 proof から関連の Read proof を作ります。
    pub fn authorize_read_from_updates(
        self,
        source: &Allowed<AnswerEntry, Update>,
        target: &Allowed<AnswerEntry, Update>,
    ) -> Result<Allowed<ReadableAnswerRelation, Read>, DomainError> {
        self.authorize_read_with_proofs(source, target)
    }
}

impl Allowed<ReadableAnswerRelation, Read> {
    /// 回答 identity だけを使って、認可済み関係の反対側を取り出します。
    ///
    /// 呼び出し側は同じ Repository 呼び出しで認可された source identity を渡します。
    pub fn opposite_endpoint_for(
        &self,
        source: AnswerReference,
    ) -> Result<AnswerReference, DomainError> {
        self.value().opposite_endpoint_for(source)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[derive(Clone, Debug, PartialEq)]
    struct TestEndpoint(AnswerReference);

    impl AnswerRelationEndpoint for TestEndpoint {
        fn answer_reference(&self) -> AnswerReference {
            self.0
        }
    }

    impl AuthorizationRole for TestEndpoint {
        type Role = SelfGuarded;
    }

    impl AuthorizationGuardDefinitions for TestEndpoint {
        fn can_create(&self, _actor: &Actor) -> bool {
            false
        }

        fn can_read(&self, _actor: &Actor) -> bool {
            true
        }

        fn can_update(&self, _actor: &Actor) -> bool {
            false
        }

        fn can_delete(&self, _actor: &Actor) -> bool {
            false
        }
    }

    fn reference(form_id: u128, answer_id: u128) -> AnswerReference {
        AnswerReference::new(
            Uuid::from_u128(form_id).into(),
            Uuid::from_u128(answer_id).into(),
        )
    }

    fn actor(seed: u128) -> Actor {
        crate::account::models::AccountUser::new(
            format!("user-{seed}"),
            Uuid::from_u128(seed).into(),
            crate::account::models::Role::StandardUser,
        )
        .into()
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

    #[test]
    fn authorize_read_rejects_different_actor_or_endpoint_proofs() {
        let source_reference = reference(1, 1);
        let target_reference = reference(2, 1);
        let relation = AnswerRelation::new(source_reference, target_reference).unwrap();
        let owner = actor(1);
        let source = AuthorizationGuard::<_, Read>::from(TestEndpoint(source_reference))
            .try_read(owner.clone())
            .unwrap();
        let target = AuthorizationGuard::<_, Read>::from(TestEndpoint(target_reference))
            .try_read(owner.clone())
            .unwrap();

        assert!(relation.authorize_read(&source, &target).is_ok());

        let different_actor_target =
            AuthorizationGuard::<_, Read>::from(TestEndpoint(target_reference))
                .try_read(actor(2))
                .unwrap();
        assert!(
            relation
                .authorize_read(&source, &different_actor_target)
                .is_err()
        );

        let different_endpoint_target =
            AuthorizationGuard::<_, Read>::from(TestEndpoint(reference(3, 1)))
                .try_read(owner)
                .unwrap();
        assert!(
            relation
                .authorize_read(&source, &different_endpoint_target)
                .is_err()
        );
    }

    #[test]
    fn readable_relation_guard_is_bound_to_the_actor_who_read_both_endpoints() {
        let relation = AnswerRelation::new(reference(1, 1), reference(2, 1)).unwrap();
        let owner: Actor = crate::account::models::AccountUser::new(
            "owner".to_string(),
            Uuid::from_u128(1).into(),
            crate::account::models::Role::StandardUser,
        )
        .into();
        let another_actor: Actor = crate::account::models::AccountUser::new(
            "another".to_string(),
            Uuid::from_u128(2).into(),
            crate::account::models::Role::StandardUser,
        )
        .into();
        let readable = ReadableAnswerRelation::new(relation, owner.clone());

        assert!(
            AuthorizationGuard::<_, Read>::from(readable.clone())
                .try_read(owner)
                .is_ok()
        );
        assert!(
            AuthorizationGuard::<_, Read>::from(readable)
                .try_read(another_actor)
                .is_err()
        );
    }
}
