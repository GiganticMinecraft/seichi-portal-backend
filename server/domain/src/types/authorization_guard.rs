use errors::domain::DomainError;

use crate::user::models::User;

pub trait Actions {}

pub struct Create;
pub struct Read;

impl Actions for Create {}
impl Actions for Read {}

/// `guard_target` を保持し、[User] が `guard_target` に対してアクセス可能かどうかを判定するための構造体
pub struct AuthorizationGuard<T: AuthorizationGuardDefinitions<T>, A: Actions> {
    guard_target: T,
    _phantom_data: std::marker::PhantomData<A>,
}

impl<T: AuthorizationGuardDefinitions<T>> AuthorizationGuard<T, Create> {
    /// [`AuthorizationGuardDefinitions::can_create`] の条件で新しい [`AuthorizationGuard`] の作成を試みます。
    pub(crate) fn try_new(actor: &User, guard_target: T) -> Result<Self, DomainError> {
        if guard_target.can_create(actor) {
            Ok(Self {
                guard_target,
                _phantom_data: std::marker::PhantomData,
            })
        } else {
            Err(DomainError::Forbidden)
        }
    }

    /// [`AuthorizationGuard`] の [`Create`] Action を持つ新しい [`AuthorizationGuard`] を作成権限を確認せずに作成します。
    ///
    /// # Safety
    /// この関数は Actor の作成権限を確認しないので、すでに永続化されたデータを読み出すときなど
    /// 作成権限を確認する必要がない場合にのみ使用してください。
    pub(crate) unsafe fn new_unchecked(guard_target: T) -> Self {
        Self {
            guard_target,
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
}

/// `actor` が `guard_target` に対して操作可能かどうかを判定するためのトレイト
///
/// # Examples
/// ```
/// use domain::{
///     message::models::{Message, MessageId},
///     types::authorization_guard::AuthorizationGuardDefinitions,
///     user::models::{Role, User},
/// };
/// use uuid::Uuid;
///
/// struct MessageGuard {
///     pub _value: String,
/// }
///
/// impl AuthorizationGuardDefinitions<Message> for MessageGuard {
///     fn can_create(&self, user: &User) -> bool {
///         user.role == Role::Administrator
///     }
///
///     fn can_read(&self, user: &User) -> bool {
///         user.role == Role::Administrator || user.role == Role::StandardUser
///     }
/// }
/// ```
pub trait AuthorizationGuardDefinitions<T> {
    fn can_create(&self, actor: &User) -> bool;
    fn can_read(&self, actor: &User) -> bool;
}

#[cfg(test)]
mod test {
    use uuid::Uuid;

    use crate::{
        types::authorization_guard::{AuthorizationGuard, AuthorizationGuardDefinitions},
        user::models::{Role, User},
    };

    #[test]
    fn authorization_guard_test() {
        struct AuthorizationGuardTestStruct {
            pub _value: String,
        }

        impl AuthorizationGuardDefinitions<AuthorizationGuardTestStruct> for AuthorizationGuardTestStruct {
            fn can_create(&self, user: &User) -> bool {
                user.role == Role::Administrator
            }

            fn can_read(&self, user: &User) -> bool {
                user.role == Role::Administrator || user.role == Role::StandardUser
            }
        }

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

        let guard_by_admin = AuthorizationGuard::try_new(
            &admin,
            AuthorizationGuardTestStruct {
                _value: "test".to_string(),
            },
        );

        let guard_by_standard_user = AuthorizationGuard::try_new(
            &standard_user,
            AuthorizationGuardTestStruct {
                _value: "test".to_string(),
            },
        );

        assert!(&guard_by_admin.is_ok());
        assert!(&guard_by_standard_user.is_err());

        let read_guard = guard_by_admin.unwrap().into_read();
        assert!(&read_guard.try_read(&admin).is_ok());
        assert!(&read_guard.try_read(&standard_user).is_ok())
    }
}
