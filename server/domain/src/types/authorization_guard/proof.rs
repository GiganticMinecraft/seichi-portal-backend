use std::marker::PhantomData;
use std::ops::Deref;

use crate::auth::Actor;

use super::Actions;

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

impl<T, A: Actions> Deref for Allowed<T, A> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
