use std::marker::PhantomData;
use std::ops::Deref;

use crate::auth::Actor;
use errors::domain::DomainError;

use super::{Actions, Create, Delete};

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
