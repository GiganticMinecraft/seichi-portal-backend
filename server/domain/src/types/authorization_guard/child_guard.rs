use errors::domain::DomainError;
use std::marker::PhantomData;

use crate::auth::Actor;

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
        C: GuardedBy<T, TargetAction>,
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
    /// 更新認可済みの親要素から、その配下に作成する子要素の認可済み値を作ります。
    pub(crate) fn authorize_create<C>(&self, child: C) -> Result<Allowed<C, Create>, DomainError>
    where
        C: GuardedBy<T, Create>,
    {
        self.authorize_any(child)
    }

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

#[cfg(test)]
mod test {
    use uuid::Uuid;

    use errors::domain::DomainError;

    use crate::{
        account::models::{AccountUser, Role},
        auth::Actor,
        types::authorization_guard::{
            Allowed, AuthorizationGuard, AuthorizationGuardDefinitions, AuthorizationRole,
            BelongsTo, Create, Delete, GuardedBy, ParentGuarded, Read, SelfGuarded, Update,
        },
    };

    #[derive(Clone, PartialEq, Debug)]
    struct ParentGuardTestStruct {
        pub id: Uuid,
    }

    impl AuthorizationRole for ParentGuardTestStruct {
        type Role = SelfGuarded;
    }

    impl AuthorizationGuardDefinitions for ParentGuardTestStruct {
        fn can_create(&self, actor: &Actor) -> bool {
            matches!(actor, Actor::AccountUser(actor) if actor.role() == &Role::Administrator)
        }

        fn can_read(&self, actor: &Actor) -> bool {
            matches!(actor, Actor::AccountUser(actor) if actor.role() == &Role::Administrator)
        }

        fn can_update(&self, actor: &Actor) -> bool {
            matches!(actor, Actor::AccountUser(actor) if actor.role() == &Role::Administrator)
        }

        fn can_delete(&self, actor: &Actor) -> bool {
            matches!(actor, Actor::AccountUser(actor) if actor.role() == &Role::Administrator)
        }
    }

    #[derive(Clone, PartialEq, Debug)]
    struct ChildGuardTestStruct {
        pub parent_id: Uuid,
        pub allowed: bool,
    }

    impl AuthorizationRole for ChildGuardTestStruct {
        type Role = ParentGuarded<ParentGuardTestStruct>;
    }

    impl BelongsTo<ParentGuardTestStruct> for ChildGuardTestStruct {
        fn belongs_to(&self, parent: &ParentGuardTestStruct) -> bool {
            self.parent_id == parent.id
        }
    }

    impl GuardedBy<ParentGuardTestStruct, Create> for ChildGuardTestStruct {
        fn is_allowed_for(&self, _parent: &ParentGuardTestStruct, _actor: &Actor) -> bool {
            self.allowed
        }
    }

    impl GuardedBy<ParentGuardTestStruct, Read> for ChildGuardTestStruct {
        fn is_allowed_for(&self, _parent: &ParentGuardTestStruct, _actor: &Actor) -> bool {
            self.allowed
        }
    }

    impl GuardedBy<ParentGuardTestStruct, Update> for ChildGuardTestStruct {
        fn is_allowed_for(&self, _parent: &ParentGuardTestStruct, _actor: &Actor) -> bool {
            self.allowed
        }
    }

    impl GuardedBy<ParentGuardTestStruct, Delete> for ChildGuardTestStruct {
        fn is_allowed_for(&self, _parent: &ParentGuardTestStruct, _actor: &Actor) -> bool {
            self.allowed
        }
    }

    #[test]
    fn allowed_parent_can_authorize_child() {
        let admin: Actor = AccountUser::new(
            "admin".to_string(),
            Uuid::new_v4().into(),
            Role::Administrator,
        )
        .into();
        let parent_id = Uuid::new_v4();
        let parent = ParentGuardTestStruct { id: parent_id };
        let child = ChildGuardTestStruct {
            parent_id,
            allowed: true,
        };

        let read_parent = AuthorizationGuard::<_, Read>::from(parent.clone())
            .try_read(admin.clone())
            .unwrap();
        let read_child = read_parent.authorize_read(child.clone()).unwrap();

        assert_eq!(read_child.value(), &child);
        assert_eq!(read_child.actor(), &admin);

        assert!(
            read_parent
                .authorize_create(child.clone())
                .map(Allowed::into_inner)
                .is_ok()
        );
        assert!(
            read_parent
                .authorize_update(child.clone())
                .map(Allowed::into_inner)
                .is_ok()
        );
        assert!(
            read_parent
                .authorize_delete(child.clone())
                .map(Allowed::into_inner)
                .is_ok()
        );

        let update_parent = AuthorizationGuard::<_, Update>::from(parent)
            .try_update(admin)
            .unwrap();

        assert!(
            update_parent
                .authorize_update(child.clone())
                .map(Allowed::into_inner)
                .is_ok()
        );
        assert!(
            update_parent
                .authorize_delete(child)
                .map(Allowed::into_inner)
                .is_ok()
        );
    }

    #[test]
    fn allowed_parent_rejects_child_with_unmatched_parent() {
        let admin: Actor = AccountUser::new(
            "admin".to_string(),
            Uuid::new_v4().into(),
            Role::Administrator,
        )
        .into();
        let parent = ParentGuardTestStruct { id: Uuid::new_v4() };
        let child = ChildGuardTestStruct {
            parent_id: Uuid::new_v4(),
            allowed: true,
        };

        let read_parent = AuthorizationGuard::<_, Read>::from(parent)
            .try_read(admin)
            .unwrap();

        assert!(matches!(
            read_parent.authorize_read(child),
            Err(DomainError::NotFound)
        ));
    }

    #[test]
    fn allowed_parent_rejects_child_when_guarded_by_denies_actor() {
        let admin: Actor = AccountUser::new(
            "admin".to_string(),
            Uuid::new_v4().into(),
            Role::Administrator,
        )
        .into();
        let parent_id = Uuid::new_v4();
        let parent = ParentGuardTestStruct { id: parent_id };
        let child = ChildGuardTestStruct {
            parent_id,
            allowed: false,
        };

        let read_parent = AuthorizationGuard::<_, Read>::from(parent)
            .try_read(admin)
            .unwrap();

        assert!(matches!(
            read_parent.authorize_read(child),
            Err(DomainError::Forbidden)
        ));
    }
}
