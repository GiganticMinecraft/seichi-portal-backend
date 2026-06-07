use errors::domain::DomainError;
use std::marker::PhantomData;

use crate::user::models::Actor;

use super::{Actions, Allowed, AuthorizationRole, Create, Delete, Read, Update};

/// 認可対象が指定した親要素の [`Allowed`] を起点として認可される子要素であることを表します。
#[derive(Debug, Clone, Copy)]
pub struct ParentGuarded<Parent>(PhantomData<fn() -> Parent>);

/// 子要素が特定の親要素に属しているかを表すトレイト
pub trait BelongsTo<Parent> {
    fn belongs_to(&self, parent: &Parent) -> bool;
}

/// 子要素が、どの親要素の認可済み値を起点に認可されるかを表すトレイト
pub trait GuardedBy<Parent, A: Actions>:
    AuthorizationRole<Role = ParentGuarded<Parent>> + BelongsTo<Parent>
{
    /// [`GuardedBy`] を実装する子要素が、[`Parent`] を起点とした [`A`] を許可するか
    fn is_allowed_for(&self, parent: &Parent, actor: &Actor) -> bool;
}

impl<T, A: Actions> Allowed<T, A> {
    /// 認可済みの親要素から、子要素の同じ操作に対する認可済み値を作ります。
    ///
    /// 子要素が実装する [`GuardedBy`] で親子関係や所有者などを確認し、
    /// 成功した場合だけ同じ [`Actor`] の [`Allowed<C, A>`] を返します。
    pub(crate) fn authorize<C>(&self, child: C) -> Result<Allowed<C, A>, DomainError>
    where
        C: GuardedBy<T, A>,
    {
        self.authorize_any(child)
    }

    /// 子要素が実装する [`GuardedBy`] で子要素を検証し、成功した場合だけ
    /// 同じ [`Actor`] の [`Allowed<C, TargetAction>`] を作る共通処理です。
    fn authorize_any<C, TargetAction>(
        &self,
        child: C,
    ) -> Result<Allowed<C, TargetAction>, DomainError>
    where
        C: GuardedBy<T, TargetAction> + BelongsTo<T>,
        TargetAction: Actions,
    {
        if !child.belongs_to(&self.value) {
            return Err(DomainError::NotFound);
        }
        if !child.is_allowed_for(&self.value, &self.actor) {
            return Err(DomainError::Forbidden);
        }
        Ok(Allowed::mint(child, self.actor.clone()))
    }
}

impl<T> Allowed<T, Update> {
    /// 更新認可済みの親要素から、子要素の更新認可済み値を作ります。
    pub(crate) fn authorize_update<C>(&self, child: C) -> Result<Allowed<C, Update>, DomainError>
    where
        C: GuardedBy<T, Update>,
    {
        self.authorize(child)
    }

    /// 更新認可済みの親要素から、子要素の削除認可済み値を作ります。
    pub(crate) fn authorize_delete<C>(&self, child: C) -> Result<Allowed<C, Delete>, DomainError>
    where
        C: GuardedBy<T, Delete>,
    {
        self.authorize_any(child)
    }
}

impl<T> Allowed<T, Read> {
    /// 読み取り認可済みの親要素から、子要素の読み取り認可済み値を作ります。
    pub(crate) fn authorize_read<C>(&self, child: C) -> Result<Allowed<C, Read>, DomainError>
    where
        C: GuardedBy<T, Read>,
    {
        self.authorize(child)
    }

    /// 読み取り認可済みの親要素から、子要素の作成認可済み値を作ります。
    ///
    /// 親要素を読める利用者に、その配下の子要素の作成を許可する場合に使います。
    pub(crate) fn authorize_create<C>(&self, child: C) -> Result<Allowed<C, Create>, DomainError>
    where
        C: GuardedBy<T, Create>,
    {
        self.authorize_any(child)
    }

    /// 読み取り認可済みの親要素から、子要素の更新認可済み値を作ります。
    pub(crate) fn authorize_update<C>(&self, child: C) -> Result<Allowed<C, Update>, DomainError>
    where
        C: GuardedBy<T, Update>,
    {
        self.authorize_any(child)
    }

    /// 読み取り認可済みの親要素から、子要素の削除認可済み値を作ります。
    pub(crate) fn authorize_delete<C>(&self, child: C) -> Result<Allowed<C, Delete>, DomainError>
    where
        C: GuardedBy<T, Delete>,
    {
        self.authorize_any(child)
    }
}
