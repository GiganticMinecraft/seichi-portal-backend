use std::marker::PhantomData;
use std::ops::Deref;

use crate::{
    auth::Actor,
    form::answer::{AnswerEntry, AnswerRelation, AnswerRelationEndpoint},
};
use errors::domain::DomainError;

use super::{Actions, Create, Delete, Read, Update};

/// 削除認可済みの値から作成できる、削除後状態を定義します。
pub(crate) trait DeleteTransition: Sized {
    type Created;
    type Context;

    fn transition(
        self,
        context: Self::Context,
        actor: &Actor,
    ) -> Result<Self::Created, DomainError>;
}

/// 指定した操作について認可済みであることを表す型
#[derive(Debug, Clone, PartialEq)]
pub struct Allowed<T, A: Actions> {
    pub(super) value: T,
    pub(super) actor: Actor,
    pub(super) _phantom_data: PhantomData<A>,
}

impl<T, A: Actions> Allowed<T, A> {
    pub(super) fn mint(value: T, actor: Actor) -> Self {
        Self {
            value,
            actor,
            _phantom_data: PhantomData,
        }
    }

    /// 認可済みの値を参照します。
    pub fn value(&self) -> &T {
        &self.value
    }

    /// 認可済みの値を取り出します。
    pub fn into_inner(self) -> T {
        self.value
    }

    /// 認可に使った [`Actor`] を参照します。
    pub fn actor(&self) -> &Actor {
        &self.actor
    }
}

impl AnswerRelation {
    /// source と target の回答 Read proof を合成し、関連自体の Read proof を作ります。
    ///
    /// Repository は対象回答の可視性を先に確認したうえでこのメソッドを呼ぶため、
    /// `Allowed<AnswerRelation, Read>` が返る時点で両端点の閲覧認可が成立しています。
    pub fn authorize_read<S, T>(
        self,
        source: &Allowed<S, Read>,
        target: &Allowed<T, Read>,
    ) -> Result<Allowed<Self, Read>, DomainError>
    where
        S: AnswerRelationEndpoint,
        T: AnswerRelationEndpoint,
    {
        if source.actor() != target.actor() {
            return Err(DomainError::InvalidEntity {
                message: "answer relation proofs must belong to the same actor".to_string(),
            });
        }

        if self.other_endpoint(source.value().answer_reference())
            != Some(target.value().answer_reference())
        {
            return Err(DomainError::InvalidEntity {
                message: "answer relation endpoints do not match authorized answers".to_string(),
            });
        }

        Ok(Allowed::mint(self, source.actor().clone()))
    }

    /// 更新認可済みの source と、対象の Read proof から関連の Read proof を作ります。
    pub fn authorize_read_from_update<T>(
        self,
        source: &Allowed<AnswerEntry, Update>,
        target: &Allowed<T, Read>,
    ) -> Result<Allowed<Self, Read>, DomainError>
    where
        T: AnswerRelationEndpoint,
    {
        if source.actor() != target.actor() {
            return Err(DomainError::InvalidEntity {
                message: "answer relation proofs must belong to the same actor".to_string(),
            });
        }

        if self.other_endpoint(source.value().answer_reference())
            != Some(target.value().answer_reference())
        {
            return Err(DomainError::InvalidEntity {
                message: "answer relation endpoints do not match authorized answers".to_string(),
            });
        }

        Ok(Allowed::mint(self, source.actor().clone()))
    }

    /// 関連追加用に、source と target の更新 proof から関連の Read proof を作ります。
    pub fn authorize_read_from_updates(
        self,
        source: &Allowed<AnswerEntry, Update>,
        target: &Allowed<AnswerEntry, Update>,
    ) -> Result<Allowed<Self, Read>, DomainError> {
        if source.actor() != target.actor() {
            return Err(DomainError::InvalidEntity {
                message: "answer relation proofs must belong to the same actor".to_string(),
            });
        }

        if self.other_endpoint(source.value().answer_reference())
            != Some(target.value().answer_reference())
        {
            return Err(DomainError::InvalidEntity {
                message: "answer relation endpoints do not match authorized answers".to_string(),
            });
        }

        Ok(Allowed::mint(self, source.actor().clone()))
    }
}

impl<T> Allowed<T, Delete> {
    /// 削除認可済みの値を消費し、削除後状態を作成する認可済みの値へ遷移します。
    ///
    /// 認可に使った [`Actor`] は遷移処理へ参照で渡した後、そのまま作成の証明へ
    /// 引き継ぎます。
    pub(crate) fn delete(
        self,
        context: T::Context,
    ) -> Result<Allowed<T::Created, Create>, DomainError>
    where
        T: DeleteTransition,
    {
        let Self { value, actor, .. } = self;
        let value = value.transition(context, &actor)?;

        Ok(Allowed::mint(value, actor))
    }
}

impl<T, A: Actions> Deref for Allowed<T, A> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        account::models::{AccountUser, Role},
        form::answer::{AnswerReference, AnswerRelationEndpoint},
    };
    use uuid::Uuid;

    #[derive(Clone, Copy)]
    struct TestAnswer(AnswerReference);

    impl AnswerRelationEndpoint for TestAnswer {
        fn answer_reference(&self) -> AnswerReference {
            self.0
        }
    }

    fn actor(seed: u128) -> Actor {
        AccountUser::new(
            format!("user-{seed}"),
            Uuid::from_u128(seed).into(),
            Role::StandardUser,
        )
        .into()
    }

    fn reference(answer_id: u128) -> AnswerReference {
        AnswerReference::new(Uuid::from_u128(1).into(), Uuid::from_u128(answer_id).into())
    }

    #[test]
    fn relation_read_proof_requires_matching_actor_and_endpoints() {
        let source_reference = reference(1);
        let target_reference = reference(2);
        let relation = AnswerRelation::new(source_reference, target_reference).unwrap();
        let first_actor = actor(1);
        let source = Allowed::mint(TestAnswer(source_reference), first_actor.clone());
        let target = Allowed::mint(TestAnswer(target_reference), first_actor.clone());

        assert!(relation.authorize_read(&source, &target).is_ok());

        let different_actor_target = Allowed::mint(TestAnswer(target_reference), actor(2));
        assert!(
            relation
                .authorize_read(&source, &different_actor_target)
                .is_err()
        );

        let different_endpoint_target =
            Allowed::mint(TestAnswer(reference(3)), first_actor.clone());
        assert!(
            relation
                .authorize_read(&source, &different_endpoint_target)
                .is_err()
        );
    }
}
