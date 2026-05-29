use errors::domain::DomainError;

use crate::user::models::Actor;

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

/// [`Actor`] による `guard_target` に対するアクセスを制御するための型です。
///
/// Action 型パラメータにより、現在許可されている操作の種類を型レベルで表現します。
// NOTE: Action の変換は以下のようにのみ行うことができます
//    - Create -> Read
//    - Create -> Update
//    - Update <-> Read
//    - Update または Read -> Delete
//  これは、データのライフサイクルを考えた時に
//    - データの新規作成(Create) -> データ読み取り(Read) <-> データ更新(Update) -> データ削除(Delete)
//  という順序のみ操作が行われるはずであるからです。
//
//  仮に Delete から Read へ変換することができるとすると、データの削除操作の実装において
//  Read 権限を保持しているかつ Delete 権限を持たないユーザーが居る場合に
//  AuthorizationGuard<T, Delete> から誤って `.into_read()` を呼び出すことで
//  Read 権限を持つユーザーによってデータが削除されるという事故が発生する可能性があります。
#[derive(Debug)]
pub struct AuthorizationGuard<T: AuthorizationGuardDefinitions, A: Actions> {
    guard_target: T,
    _phantom_data: std::marker::PhantomData<A>,
}

impl<T: AuthorizationGuardDefinitions> AuthorizationGuard<T, Create> {
    pub(crate) fn new(guard_target: T) -> Self {
        Self {
            guard_target,
            _phantom_data: std::marker::PhantomData,
        }
    }

    /// [`AuthorizationGuardDefinitions::can_create`] の条件で作成操作 `f` を試みます。
    pub fn try_create<'a, R, F>(&'a self, actor: &Actor, f: F) -> Result<R, DomainError>
    where
        F: FnOnce(&'a T) -> R,
    {
        if self.guard_target.can_create(actor) {
            Ok(f(&self.guard_target))
        } else {
            Err(DomainError::Forbidden)
        }
    }

    /// [`AuthorizationGuardDefinitions::can_create`] の条件で作成操作 `f` を試みます。
    /// この関数は、`guard_target` を所有権を持つ形で操作を行います。
    pub fn try_into_create<R, F>(self, actor: &Actor, f: F) -> Result<R, DomainError>
    where
        F: FnOnce(T) -> R,
    {
        if self.guard_target.can_create(actor) {
            Ok(f(self.guard_target))
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

impl<T: AuthorizationGuardDefinitions> AuthorizationGuard<T, Update> {
    /// [`AuthorizationGuardDefinitions::can_update`] の条件で更新操作 `f` を試みます。
    pub fn try_update<'a, R, F>(&'a self, actor: &Actor, f: F) -> Result<R, DomainError>
    where
        F: FnOnce(&'a T) -> R,
    {
        if self.guard_target.can_update(actor) {
            Ok(f(&self.guard_target))
        } else {
            Err(DomainError::Forbidden)
        }
    }

    /// [`AuthorizationGuardDefinitions::can_update`] の条件で更新操作 `f` を試みます。
    /// この関数は、`guard_target` を所有権を持つ形で操作を行います。
    pub fn try_into_update<R, F>(self, actor: &Actor, f: F) -> Result<R, DomainError>
    where
        F: FnOnce(T) -> R,
    {
        if self.guard_target.can_update(actor) {
            Ok(f(self.guard_target))
        } else {
            Err(DomainError::Forbidden)
        }
    }

    /// [`T`] の値に対して map 相当の操作を行います。
    pub fn map<F>(self, f: F) -> AuthorizationGuard<T, Update>
    where
        F: FnOnce(T) -> T,
    {
        AuthorizationGuard {
            guard_target: f(self.guard_target),
            _phantom_data: std::marker::PhantomData,
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

impl<T: AuthorizationGuardDefinitions> AuthorizationGuard<T, Read> {
    /// `actor` が `guard_target` の参照を取得することを試みます。
    pub fn try_read(&self, actor: &Actor) -> Result<&T, DomainError> {
        if self.guard_target.can_read(actor) {
            Ok(&self.guard_target)
        } else {
            Err(DomainError::Forbidden)
        }
    }

    /// `actor` が `guard_target` を取得することを試みます。
    pub fn try_into_read(self, actor: &Actor) -> Result<T, DomainError> {
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

impl<T: AuthorizationGuardDefinitions> AuthorizationGuard<T, Delete> {
    /// [`AuthorizationGuardDefinitions::can_delete`] の条件で削除操作 `f` を試みます。
    pub fn try_delete<'a, R, F>(&'a self, actor: &Actor, f: F) -> Result<R, DomainError>
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
    pub fn try_into_delete<R, F>(self, actor: &Actor, f: F) -> Result<R, DomainError>
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
/// # Examples
/// ```
/// use domain::{
///     types::authorization_guard::AuthorizationGuardDefinitions,
///     user::models::{Actor, Role, User, UserId},
/// };
/// use uuid::Uuid;
///
/// struct GuardTarget {
///     pub user_id: UserId,
/// }
///
/// impl AuthorizationGuardDefinitions for GuardTarget {
///     fn can_create(&self, actor: &Actor) -> bool {
///         matches!(actor, Actor::User(User::ActiveUser(u)) if u.role() == &Role::Administrator)
///     }
///
///     fn can_read(&self, actor: &Actor) -> bool {
///         matches!(actor, Actor::User(User::ActiveUser(u)) if *u.id() == self.user_id)
///     }
///
///     fn can_update(&self, actor: &Actor) -> bool {
///         matches!(actor, Actor::User(User::ActiveUser(u)) if *u.id() == self.user_id)
///     }
///
///     fn can_delete(&self, actor: &Actor) -> bool {
///         matches!(actor, Actor::User(User::ActiveUser(u)) if *u.id() == self.user_id)
///     }
/// }
/// ```
pub trait AuthorizationGuardDefinitions {
    fn can_create(&self, actor: &Actor) -> bool;
    fn can_read(&self, actor: &Actor) -> bool;
    fn can_update(&self, actor: &Actor) -> bool;
    fn can_delete(&self, actor: &Actor) -> bool;
}

impl<T: AuthorizationGuardDefinitions> From<T> for AuthorizationGuard<T, Create> {
    fn from(guard_target: T) -> Self {
        AuthorizationGuard::new(guard_target)
    }
}

impl<T: AuthorizationGuardDefinitions> From<T> for AuthorizationGuard<T, Read> {
    fn from(guard_target: T) -> Self {
        Self {
            guard_target,
            _phantom_data: std::marker::PhantomData,
        }
    }
}

impl<T: AuthorizationGuardDefinitions> From<T> for AuthorizationGuard<T, Update> {
    fn from(guard_target: T) -> Self {
        Self {
            guard_target,
            _phantom_data: std::marker::PhantomData,
        }
    }
}

impl<T: AuthorizationGuardDefinitions> From<T> for AuthorizationGuard<T, Delete> {
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
        user::models::{ActiveUser, Actor, Role, User},
    };

    #[derive(Clone, PartialEq, Debug)]
    struct AuthorizationGuardTestStruct {
        pub _value: String,
    }

    impl AuthorizationGuardDefinitions for AuthorizationGuardTestStruct {
        fn can_create(&self, actor: &Actor) -> bool {
            matches!(actor, Actor::User(User::ActiveUser(actor)) if actor.role() == &Role::Administrator)
        }

        fn can_read(&self, actor: &Actor) -> bool {
            matches!(
                actor,
                Actor::User(User::ActiveUser(actor))
                    if actor.role() == &Role::Administrator
                        || actor.role() == &Role::StandardUser
            )
        }

        fn can_update(&self, actor: &Actor) -> bool {
            matches!(actor, Actor::User(User::ActiveUser(actor)) if actor.role() == &Role::Administrator)
        }

        fn can_delete(&self, actor: &Actor) -> bool {
            matches!(actor, Actor::User(User::ActiveUser(actor)) if actor.role() == &Role::Administrator)
        }
    }

    #[test]
    fn authorization_guard_test() {
        let admin: Actor = ActiveUser::new(
            "admin".to_string(),
            Uuid::new_v4().into(),
            Role::Administrator,
        )
        .into();

        let standard_user: Actor = ActiveUser::new(
            "standard_user".to_string(),
            Uuid::new_v4().into(),
            Role::StandardUser,
        )
        .into();

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
        let user: Actor = ActiveUser::new(
            "user".to_string(),
            Uuid::new_v4().into(),
            Role::Administrator,
        )
        .into();

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
