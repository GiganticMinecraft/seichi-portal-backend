use errors::domain::DomainError;

use crate::user::models::User;

pub trait Actions {}

pub struct Create;
pub struct Read;
pub struct Delete;

impl Actions for Create {}
impl Actions for Read {}
impl Actions for Delete {}

/// [`User`] の `guard_target` に対するアクセスを制御するための定義を提供します。
///
/// この定義は、`guard_target` によってアクセス権が異なるデータの操作を制御することのみを想定しています。
pub struct AuthorizationGuard<T: AuthorizationGuardDefinitions<T>, A: Actions> {
    guard_target: T,
    _phantom_data: std::marker::PhantomData<A>,
}

// NOTE: 実装時点(2024/10/21)では、AuthorizationGuard の Action は Create から Read への変換、
//  Read から Delete への変換のみ変換されると定義しています。
//  これは、データのライフサイクルを考えた時に
//      - データの新規作成(Create)
//      - 永続化データ詰め込み(Create) -> 詰め込みデータ読み取り(Read)
//      - 永続化データ詰め込み(Create) -> (詰め込みデータ読み取り(Read) ->) 詰め込みデータ削除(Delete)
//  という3つの操作以外は望ましくない(実装されるべきではない)と考えているためです。
//  Delete から Read へ変換することができると仮定すると、 データの削除操作の実装において
//  Read 権限を保持しているかつ、Delete 権限を持たないユーザーが居る場合に
//  AuthorizationGuard<T, Delete> から誤って `.into_read()` 関数を呼び出すことで
//  Read 権限を持つユーザーによってデータが削除されるという事故が発生する可能性があります。
//
//  これは、データの削除を担当する repository の関数において、
//  AuthorizationGuard<T, Delete> を引数に受け取る関数が定義されることを想定しています。
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

    /// [`AuthorizationGuard`] の Action を [`Delete`] に変換します。
    pub fn into_delete(self) -> AuthorizationGuard<T, Delete> {
        AuthorizationGuard {
            guard_target: self.guard_target,
            _phantom_data: std::marker::PhantomData,
        }
    }
}

impl<T: AuthorizationGuardDefinitions<T>> AuthorizationGuard<T, Delete> {
    /// `actor` が `guard_target` を削除するための情報を取得することを試みます。
    pub fn try_delete(&self, actor: &User) -> Result<&T, DomainError> {
        if self.guard_target.can_delete(actor) {
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
///     fn can_delete(&self, actor: &User) -> bool {
///         self.user.id == actor.id
///     }
/// }
/// ```
pub trait AuthorizationGuardDefinitions<T> {
    fn can_create(&self, actor: &User) -> bool;
    fn can_read(&self, actor: &User) -> bool;
    fn can_delete(&self, actor: &User) -> bool;
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

            fn can_delete(&self, actor: &User) -> bool {
                actor.role == Role::Administrator
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
