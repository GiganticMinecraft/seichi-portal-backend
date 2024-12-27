use errors::domain::DomainError;

use crate::user::models::User;

pub trait Actions: private::Sealed {}

#[derive(Debug)]
pub struct Create;
#[derive(Debug)]
pub struct Read;
#[derive(Debug)]
pub struct Update;
#[derive(Debug)]
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

/// [`User`] による `guard_target` に対するアクセスを制御するための定義を提供します。
#[derive(Debug)]
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
    pub fn try_update<'a, R, F>(&'a self, actor: &User, f: F) -> Result<R, DomainError>
    where
        F: FnOnce(&'a T) -> R,
    {
        if self.guard_target.can_update(actor) {
            Ok(f(&self.guard_target))
        } else {
            Err(DomainError::Forbidden)
        }
    }

    /// [`T`] の値に対して map 相当の操作を行います。
    ///
    /// [`actor`] に [`UPDATE`] 権限がある場合は [`f`] を適用し、
    /// そうでない場合は [`self`] をそのまま返します。
    pub fn map<F>(self, actor: &User, f: F) -> AuthorizationGuard<T, Update>
    where
        F: FnOnce(T) -> T,
    {
        if self.guard_target.can_update(actor) {
            AuthorizationGuard {
                guard_target: f(self.guard_target),
                _phantom_data: std::marker::PhantomData,
            }
        } else {
            self
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
    /// `actor` が `guard_target` の参照を取得することを試みます。
    pub fn try_read(&self, actor: &User) -> Result<&T, DomainError> {
        if self.guard_target.can_read(actor) {
            Ok(&self.guard_target)
        } else {
            Err(DomainError::Forbidden)
        }
    }

    /// `actor` が `guard_target` を取得することを試みます。
    pub fn try_into_read(self, actor: &User) -> Result<T, DomainError> {
        if self.guard_target.can_read(actor) {
            Ok(self.guard_target)
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

    /// [`AuthorizationGuardDefinitions::can_delete`] の条件で削除操作 `f` を試みます。
    /// この関数は、`guard_target` を所有権を持つ形で操作を行います。
    pub fn try_into_delete<R, F>(self, actor: &User, f: F) -> Result<R, DomainError>
    where
        F: FnOnce(T) -> R,
    {
        if self.guard_target.can_delete(actor) {
            Ok(f(self.guard_target))
        } else {
            Err(DomainError::Forbidden)
        }
    }
}

/// `actor` が `guard_target` に対して操作可能かどうかを定義するためのトレイト
///
/// このトレイトでは、あくまで「[`actor`] と `guard_target` の情報を使用して判断できる中で、
/// `guard_target` にアクセスすることができるかどうか」
/// という情報を提供するためのみに使用することを想定しています。
/// そのため、`guard_target` のドメイン制約に関する情報を定義することは想定していません。
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

impl<T: AuthorizationGuardDefinitions<T>> From<T> for AuthorizationGuard<T, Create> {
    fn from(guard_target: T) -> Self {
        AuthorizationGuard::new(guard_target)
    }
}

impl<T: AuthorizationGuardDefinitions<T>> From<T> for AuthorizationGuard<T, Read> {
    fn from(guard_target: T) -> Self {
        Self {
            guard_target,
            _phantom_data: std::marker::PhantomData,
        }
    }
}

impl<T: AuthorizationGuardDefinitions<T>> From<T> for AuthorizationGuard<T, Update> {
    fn from(guard_target: T) -> Self {
        Self {
            guard_target,
            _phantom_data: std::marker::PhantomData,
        }
    }
}

impl<T: AuthorizationGuardDefinitions<T>> From<T> for AuthorizationGuard<T, Delete> {
    fn from(guard_target: T) -> Self {
        Self {
            guard_target,
            _phantom_data: std::marker::PhantomData,
        }
    }
}

#[cfg(test)]
mod test {
    use uuid::Uuid;

    use crate::{
        types::authorization_guard::{AuthorizationGuard, AuthorizationGuardDefinitions},
        user::models::{Role, User},
    };

    #[derive(Clone, PartialEq, Debug)]
    struct AuthorizationGuardTestStruct {
        pub _value: String,
    }

    impl AuthorizationGuardDefinitions<AuthorizationGuardTestStruct> for AuthorizationGuardTestStruct {
        fn can_create(&self, actor: &User) -> bool {
            actor.role == Role::Administrator
        }

        fn can_read(&self, actor: &User) -> bool {
            actor.role == Role::Administrator || actor.role == Role::StandardUser
        }

        fn can_update(&self, actor: &User) -> bool {
            actor.role == Role::Administrator
        }

        fn can_delete(&self, actor: &User) -> bool {
            actor.role == Role::Administrator
        }
    }

    #[test]
    fn authorization_guard_test() {
        let admin = User {
            name: "admin".to_string(),
            id: Uuid::new_v4(),
            role: Role::Administrator,
        };

        let standard_user = User {
            name: "standard_user".to_string(),
            id: Uuid::new_v4(),
            role: Role::StandardUser,
        };

        let guard = AuthorizationGuard::new(AuthorizationGuardTestStruct {
            _value: "test".to_string(),
        });

        assert!(&guard.try_create(&admin, |_| {}).is_ok());
        assert!(&guard.try_create(&standard_user, |_| {}).is_err());

        let guard = guard.into_read();
        assert!(&guard.try_read(&admin).is_ok());
        assert!(&guard.try_read(&standard_user).is_ok());

        let guard = guard.into_update();
        assert!(&guard.try_update(&admin, |_| {}).is_ok());
        assert!(&guard.try_update(&standard_user, |_| {}).is_err());

        let guard = guard.into_delete();
        assert!(&guard.try_delete(&admin, |_| {}).is_ok());
        assert!(&guard.try_delete(&standard_user, |_| {}).is_err());
    }

    #[test]
    fn verify_same_data_for_try_read_and_try_into_read() {
        let user = User {
            name: "user".to_string(),
            id: Uuid::new_v4(),
            role: Role::Administrator,
        };

        let guard = AuthorizationGuard::new(AuthorizationGuardTestStruct {
            _value: "test".to_string(),
        });

        let read_guard = guard.into_read();

        let from_into_read = read_guard.try_read(&user);
        assert!(from_into_read.is_ok());

        let from_into_read = from_into_read.unwrap().to_owned();

        let read_into = read_guard.try_into_read(&user);

        assert!(read_into.is_ok());

        assert_eq!(from_into_read, read_into.unwrap());
    }
}
