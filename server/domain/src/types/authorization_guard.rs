use errors::domain::DomainError;

use crate::{
    types::authorization_guard_with_context::{
        Actions, AuthorizationGuardWithContext, AuthorizationGuardWithContextDefinitions, Create,
        Delete, Read, Update,
    },
    user::models::User,
};

/// [`User`] による `guard_target` に対するアクセスを制御するための定義を提供します。
///
/// [`AuthorizationGuard`] は、Context を必要としない [`AuthorizationGuardWithContext`] であり、
/// [`AuthorizationGuardWithContext<T, A, ()>`] と同等の機能を提供します。
#[derive(Debug)]
pub struct AuthorizationGuard<T: AuthorizationGuardDefinitions, A: Actions> {
    authorization_guard_with_context: AuthorizationGuardWithContext<T, A, ()>,
}

impl<T: AuthorizationGuardDefinitions> AuthorizationGuard<T, Create> {
    pub(crate) fn new(guard_target: T) -> Self {
        Self {
            authorization_guard_with_context: AuthorizationGuardWithContext::new(guard_target),
        }
    }

    /// [`AuthorizationGuardDefinitions::can_create`] の条件で作成操作 `f` を試みます。
    pub fn try_create<'a, R, F>(&'a self, actor: &User, f: F) -> Result<R, DomainError>
    where
        F: FnOnce(&'a T) -> R,
    {
        self.authorization_guard_with_context
            .try_create(actor, f, &())
    }

    /// [`AuthorizationGuardDefinitions::can_create`] の条件で作成操作 `f` を試みます。
    /// この関数は、`guard_target` を所有権を持つ形で操作を行います。
    pub fn try_into_create<R, F>(self, actor: &User, f: F) -> Result<R, DomainError>
    where
        F: FnOnce(T) -> R,
    {
        self.authorization_guard_with_context
            .try_into_create(actor, f, &())
    }

    /// [`AuthorizationGuard`] の Action を [`Read`] に変換します。
    pub fn into_read(self) -> AuthorizationGuard<T, Read> {
        AuthorizationGuard {
            authorization_guard_with_context: self.authorization_guard_with_context.into_read(),
        }
    }

    pub fn into_update(self) -> AuthorizationGuard<T, Update> {
        AuthorizationGuard {
            authorization_guard_with_context: self.authorization_guard_with_context.into_update(),
        }
    }
}

impl<T: AuthorizationGuardDefinitions> AuthorizationGuard<T, Update> {
    /// [`AuthorizationGuardDefinitions::can_update`] の条件で更新操作 `f` を試みます。
    pub fn try_update<'a, R, F>(&'a self, actor: &User, f: F) -> Result<R, DomainError>
    where
        F: FnOnce(&'a T) -> R,
    {
        self.authorization_guard_with_context
            .try_update(actor, f, &())
    }

    /// [`AuthorizationGuardDefinitions::can_update`] の条件で更新操作 `f` を試みます。
    /// この関数は、`guard_target` を所有権を持つ形で操作を行います。
    pub fn try_into_update<R, F>(self, actor: &User, f: F) -> Result<R, DomainError>
    where
        F: FnOnce(T) -> R,
    {
        self.authorization_guard_with_context
            .try_into_update(actor, f, &())
    }

    /// [`T`] の値に対して map 相当の操作を行います。
    pub fn map<F>(self, f: F) -> AuthorizationGuard<T, Update>
    where
        F: FnOnce(T) -> T,
    {
        AuthorizationGuard {
            authorization_guard_with_context: self.authorization_guard_with_context.map(f),
        }
    }

    /// [`AuthorizationGuard`] の Action を [`Read`] に変換します。
    pub fn into_read(self) -> AuthorizationGuard<T, Read> {
        AuthorizationGuard {
            authorization_guard_with_context: self.authorization_guard_with_context.into_read(),
        }
    }

    /// [`AuthorizationGuard`] の Action を [`Delete`] に変換します。
    pub fn into_delete(self) -> AuthorizationGuard<T, Delete> {
        AuthorizationGuard {
            authorization_guard_with_context: self.authorization_guard_with_context.into_delete(),
        }
    }
}

impl<T: AuthorizationGuardDefinitions> AuthorizationGuard<T, Read> {
    /// `actor` が `guard_target` の参照を取得することを試みます。
    pub fn try_read(&self, actor: &User) -> Result<&T, DomainError> {
        self.authorization_guard_with_context.try_read(actor, &())
    }

    /// `actor` が `guard_target` を取得することを試みます。
    pub fn try_into_read(self, actor: &User) -> Result<T, DomainError> {
        self.authorization_guard_with_context
            .try_into_read(actor, &())
    }

    /// [`AuthorizationGuard`] の Action を [`Update`] に変換します。
    pub fn into_update(self) -> AuthorizationGuard<T, Update> {
        AuthorizationGuard {
            authorization_guard_with_context: self.authorization_guard_with_context.into_update(),
        }
    }

    /// [`AuthorizationGuard`] の Action を [`Delete`] に変換します。
    pub fn into_delete(self) -> AuthorizationGuard<T, Delete> {
        AuthorizationGuard {
            authorization_guard_with_context: self.authorization_guard_with_context.into_delete(),
        }
    }

    /// 認可処理を行わずに、`guard_target` の参照を取得します。
    ///
    /// # Safety
    /// システム側で実行する処理で、認可を必要としない場合にのみ使用してください。
    pub unsafe fn read_unchecked(&self) -> &T {
        unsafe { self.authorization_guard_with_context.read_unchecked() }
    }

    /// 認可処理を行わずに、所有権を含めて `guard_target` を取得します。
    ///
    /// # Safety
    /// システム側で実行する処理で、認可を必要としない場合にのみ使用してください。
    pub unsafe fn into_read_unchecked(self) -> T {
        unsafe { self.authorization_guard_with_context.into_read_unchecked() }
    }
}

impl<T: AuthorizationGuardDefinitions> AuthorizationGuard<T, Delete> {
    /// [`AuthorizationGuardDefinitions::can_delete`] の条件で削除操作 `f` を試みます。
    pub fn try_delete<'a, R, F>(&'a self, actor: &User, f: F) -> Result<R, DomainError>
    where
        F: FnOnce(&'a T) -> R,
    {
        self.authorization_guard_with_context
            .try_delete(actor, f, &())
    }

    /// [`AuthorizationGuardDefinitions::can_delete`] の条件で削除操作 `f` を試みます。
    /// この関数は、`guard_target` を所有権を持つ形で操作を行います。
    pub fn try_into_delete<R, F>(self, actor: &User, f: F) -> Result<R, DomainError>
    where
        F: FnOnce(T) -> R,
    {
        self.authorization_guard_with_context
            .try_into_delete(actor, f, &())
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
/// impl AuthorizationGuardDefinitions for GuardTarget {
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
pub trait AuthorizationGuardDefinitions {
    fn can_create(&self, actor: &User) -> bool;
    fn can_read(&self, actor: &User) -> bool;
    fn can_update(&self, actor: &User) -> bool;
    fn can_delete(&self, actor: &User) -> bool;
}

impl<T> AuthorizationGuardWithContextDefinitions<()> for T
where
    T: AuthorizationGuardDefinitions,
{
    fn can_create(&self, actor: &User, _context: &()) -> bool {
        self.can_create(actor)
    }

    fn can_read(&self, actor: &User, _context: &()) -> bool {
        self.can_read(actor)
    }

    fn can_update(&self, actor: &User, _context: &()) -> bool {
        self.can_update(actor)
    }

    fn can_delete(&self, actor: &User, _context: &()) -> bool {
        self.can_delete(actor)
    }
}

impl<T: AuthorizationGuardDefinitions> From<T> for AuthorizationGuard<T, Create> {
    fn from(guard_target: T) -> Self {
        AuthorizationGuard::new(guard_target)
    }
}

impl<T: AuthorizationGuardDefinitions> From<T> for AuthorizationGuard<T, Read> {
    fn from(guard_target: T) -> Self {
        Self {
            authorization_guard_with_context: AuthorizationGuardWithContext::new(guard_target)
                .into_read(),
        }
    }
}

impl<T: AuthorizationGuardDefinitions> From<T> for AuthorizationGuard<T, Update> {
    fn from(guard_target: T) -> Self {
        Self {
            authorization_guard_with_context: AuthorizationGuardWithContext::new(guard_target)
                .into_update(),
        }
    }
}

impl<T: AuthorizationGuardDefinitions> From<T> for AuthorizationGuard<T, Delete> {
    fn from(guard_target: T) -> Self {
        Self {
            authorization_guard_with_context: AuthorizationGuardWithContext::new(guard_target)
                .into_read()
                .into_delete(),
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

    impl AuthorizationGuardDefinitions for AuthorizationGuardTestStruct {
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
