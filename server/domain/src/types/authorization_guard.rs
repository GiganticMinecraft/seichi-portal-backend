use std::future::Future;

use errors::domain::DomainError;

use crate::user::models::User;

pub trait Actions: private::Sealed {}

pub struct Create;
pub struct Read;
pub struct Update;
pub struct Delete;

impl Actions for Create {}
impl Actions for Read {}
impl Actions for Update {}
impl Actions for Delete {}

mod private {
    pub trait Sealed {}

    impl Sealed for super::Create {}
    impl Sealed for super::Read {}
    impl Sealed for super::Update {}
    impl Sealed for super::Delete {}
}

/// [`User`] の `guard_target` に対するアクセスを制御するための定義を提供します。
///
/// この定義は、`guard_target` によってアクセス権が異なるデータの操作を制御することのみを想定しています。
pub struct AuthorizationGuard<T: AuthorizationGuardDefinitions<T>, A: Actions> {
    guard_target: T,
    _phantom_data: std::marker::PhantomData<A>,
}

// NOTE: 実装時点(2024/10/27)では、AuthorizationGuard の Action は以下のようにのみ変換することができます
//    - Create -> Read
//    - Create -> Update
//    - Update <-> Read
//    - Update または Read -> Delete
//  これは、データのライフサイクルを考えた時に
//    - データの新規作成(Create) -> データ読み取り(Read) <-> データ更新(Update) -> データ削除(Delete)
//  という順序のみ操作が行われるはずであるからです。
//
//  仮に Delete から Read へ変換することができるとすると、 データの削除操作の実装において
//  Read 権限を保持しているかつ、Delete 権限を持たないユーザーが居る場合に
//  AuthorizationGuard<T, Delete> から誤って `.into_read()` 関数を呼び出すことで
//  Read 権限を持つユーザーによってデータが削除されるという事故が発生する可能性があります。
//  このような事故を防ぐために、AuthorizationGuard の Action の変換は上記のように限定されています。
impl<T: AuthorizationGuardDefinitions<T>> AuthorizationGuard<T, Create> {
    pub(crate) fn new(guard_target: T) -> Self {
        Self {
            guard_target,
            _phantom_data: std::marker::PhantomData,
        }
    }

    /// [`AuthorizationGuardDefinitions::can_create`] の条件で作成操作 `f` を試みます。
    pub fn try_create<'a, R, F>(&'a self, actor: &User, f: F) -> Result<R, DomainError>
    where
        F: FnOnce(&'a T) -> R,
    {
        if self.guard_target.can_create(actor) {
            Ok(f(&self.guard_target))
        } else {
            Err(DomainError::Forbidden)
        }
    }

    /// [`AuthorizationGuard`] の Action を [`Read`] に変換します。
    pub fn into_read(self) -> AuthorizationGuard<T, Read> {
        AuthorizationGuard {
            guard_target: self.guard_target,
            _phantom_data: std::marker::PhantomData,
        }
    }

    pub fn into_update(self) -> AuthorizationGuard<T, Update> {
        AuthorizationGuard {
            guard_target: self.guard_target,
            _phantom_data: std::marker::PhantomData,
        }
    }
}

impl<T: AuthorizationGuardDefinitions<T>> AuthorizationGuard<T, Update> {
    /// [`AuthorizationGuardDefinitions::can_update`] の条件で更新操作 `f` を試みます。
    pub async fn try_update<R, F, Fut>(&self, actor: &User, f: F) -> Result<R, DomainError>
    where
        Fut: Future<Output = R>,
        F: FnOnce(&T) -> Fut,
    {
        if self.guard_target.can_update(actor) {
            Ok(f(&self.guard_target).await)
        } else {
            Err(DomainError::Forbidden)
        }
    }

    /// [`AuthorizationGuard`] の Action を [`Read`] に変換します。
    pub fn into_read(self) -> AuthorizationGuard<T, Read> {
        AuthorizationGuard {
            guard_target: self.guard_target,
            _phantom_data: std::marker::PhantomData,
        }
    }

    /// [`AuthorizationGuard`] の Action を [`Delete`] に変換します。
    pub fn into_delete(self) -> AuthorizationGuard<T, Delete> {
        AuthorizationGuard {
            guard_target: self.guard_target,
            _phantom_data: std::marker::PhantomData,
        }
    }
}

impl<T: AuthorizationGuardDefinitions<T>> AuthorizationGuard<T, Read> {
    /// `actor` が `guard_target` を取得することを試みます。
    pub fn try_read(&self, actor: &User) -> Result<&T, DomainError> {
        if self.guard_target.can_read(actor) {
            Ok(&self.guard_target)
        } else {
            Err(DomainError::Forbidden)
        }
    }

    /// [`AuthorizationGuard`] の Action を [`Update`] に変換します。
    pub fn into_update(self) -> AuthorizationGuard<T, Update> {
        AuthorizationGuard {
            guard_target: self.guard_target,
            _phantom_data: std::marker::PhantomData,
        }
    }

    /// [`AuthorizationGuard`] の Action を [`Delete`] に変換します。
    pub fn into_delete(self) -> AuthorizationGuard<T, Delete> {
        AuthorizationGuard {
            guard_target: self.guard_target,
            _phantom_data: std::marker::PhantomData,
        }
    }
}

impl<T: AuthorizationGuardDefinitions<T>> AuthorizationGuard<T, Delete> {
    /// [`AuthorizationGuardDefinitions::can_delete`] の条件で削除操作 `f` を試みます。
    pub fn try_delete<'a, R, F>(&'a self, actor: &User, f: F) -> Result<R, DomainError>
    where
        F: FnOnce(&'a T) -> R,
    {
        if self.guard_target.can_delete(actor) {
            Ok(f(&self.guard_target))
        } else {
            Err(DomainError::Forbidden)
        }
    }
}

/// `actor` が `guard_target` に対して操作可能かどうかを定義するためのトレイト
///
/// # Examples
/// ```
/// use domain::{
///     types::authorization_guard::AuthorizationGuardDefinitions,
///     user::models::{Role, User},
/// };
/// use uuid::Uuid;
///
/// struct GuardTarget {
///     pub user: User,
/// }
///
/// impl AuthorizationGuardDefinitions<GuardTarget> for GuardTarget {
///     fn can_create(&self, actor: &User) -> bool {
///         actor.role == Role::Administrator
///     }
///
///     fn can_read(&self, actor: &User) -> bool {
///         self.user.id == actor.id
///     }
///
///     fn can_update(&self, actor: &User) -> bool {
///         self.user.id == actor.id
///     }
///
///     fn can_delete(&self, actor: &User) -> bool {
///         self.user.id == actor.id
///     }
/// }
/// ```
pub trait AuthorizationGuardDefinitions<T> {
    fn can_create(&self, actor: &User) -> bool;
    fn can_read(&self, actor: &User) -> bool;
    fn can_update(&self, actor: &User) -> bool;
    fn can_delete(&self, actor: &User) -> bool;
}

#[cfg(test)]
mod test {
    // use uuid::Uuid;
    //
    // use crate::{
    //     types::authorization_guard::{AuthorizationGuard, AuthorizationGuardDefinitions},
    //     user::models::{Role, User},
    // };

    // #[tokio::test]
    // async fn authorization_guard_test() {
    //     struct AuthorizationGuardTestStruct {
    //         pub _value: String,
    //     }
    //
    //     impl AuthorizationGuardDefinitions<AuthorizationGuardTestStruct> for AuthorizationGuardTestStruct {
    //         fn can_create(&self, actor: &User) -> bool {
    //             actor.role == Role::Administrator
    //         }
    //
    //         fn can_read(&self, actor: &User) -> bool {
    //             actor.role == Role::Administrator || actor.role == Role::StandardUser
    //         }
    //
    //         fn can_update(&self, actor: &User) -> bool {
    //             actor.role == Role::Administrator
    //         }
    //
    //         fn can_delete(&self, actor: &User) -> bool {
    //             actor.role == Role::Administrator
    //         }
    //     }
    //
    //     let admin = User {
    //         name: "admin".to_string(),
    //         id: Uuid::new_v4(),
    //         role: Role::Administrator,
    //     };
    //
    //     let standard_user = User {
    //         name: "standard_user".to_string(),
    //         id: Uuid::new_v4(),
    //         role: Role::StandardUser,
    //     };
    //
    //     let guard = AuthorizationGuard::new(AuthorizationGuardTestStruct {
    //         _value: "test".to_string(),
    //     });
    //
    //     assert!(&guard.try_create(&admin, |_| async {}).await.is_ok());
    //     assert!(&guard
    //         .try_create(&standard_user, |_| async {})
    //         .await
    //         .is_err());
    //
    //     let guard = guard.into_read();
    //     assert!(&guard.try_read(&admin).is_ok());
    //     assert!(&guard.try_read(&standard_user).is_ok())
    // }
}
