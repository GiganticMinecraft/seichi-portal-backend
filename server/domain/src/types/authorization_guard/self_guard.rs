use errors::domain::DomainError;
use std::marker::PhantomData;

use crate::user::models::Actor;

use super::{Actions, Allowed, AuthorizationRole, Create, Delete, Read, Update};

/// 認可対象が自分自身を直接ガードするルート集約であることを表します。
#[derive(Debug, Clone, Copy)]
pub struct SelfGuarded;

/// 認可チェック前の値を、実行したい操作と一緒に保持する型です。
///
/// リポジトリなどから取得した値や、これから保存したい値をまずこの型で包み、
/// `try_create` / `try_read` / `try_update` / `try_delete` のいずれかで [`Actor`] に対する
/// 認可条件を確認します。認可に成功した場合だけ [`Allowed`] が返るため、
/// 永続化や子要素の取得など、認可済みの値だけを受け付けたい API では
/// [`Allowed<T, A>`] を引数にすることでチェック漏れを型で防ぎます。
///
/// Action 型パラメータは、これから確認する操作の種類を型レベルで表します。
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
#[derive(Debug, Clone)]
pub struct AuthorizationGuard<T: AuthorizationGuardDefinitions, A: Actions> {
    guard_target: T,
    _phantom_data: PhantomData<A>,
}

/// `actor` が `guard_target` に対して操作可能かどうかを定義するためのトレイト
///
/// # Examples
/// ```
/// use domain::{
///     types::authorization_guard::{
///         AuthorizationGuardDefinitions, AuthorizationRole, SelfGuarded,
///     },
///     user::models::{Actor, Role, User, UserId},
/// };
/// use uuid::Uuid;
///
/// struct GuardTarget {
///     pub user_id: UserId,
/// }
///
/// impl AuthorizationRole for GuardTarget {
///     type Role = SelfGuarded;
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
pub trait AuthorizationGuardDefinitions: AuthorizationRole<Role = SelfGuarded> {
    fn can_create(&self, actor: &Actor) -> bool;
    fn can_read(&self, actor: &Actor) -> bool;
    fn can_update(&self, actor: &Actor) -> bool;
    fn can_delete(&self, actor: &Actor) -> bool;
}

impl<T> Allowed<T, Update>
where
    T: AuthorizationGuardDefinitions,
{
    /// 認可済みの値を更新し、同じ操作権限と [`Actor`] を引き継ぎます。
    pub fn map<F>(self, f: F) -> Self
    where
        F: FnOnce(T) -> T,
    {
        Self {
            value: f(self.value),
            actor: self.actor,
            _phantom_data: PhantomData,
        }
    }

    /// 認可済みの値に対して失敗しうる更新を行い、同じ操作権限と [`Actor`] を引き継ぎます。
    pub fn try_map<F>(self, f: F) -> Result<Self, DomainError>
    where
        F: FnOnce(T) -> Result<T, DomainError>,
    {
        Ok(Self {
            value: f(self.value)?,
            actor: self.actor,
            _phantom_data: PhantomData,
        })
    }
}

impl<T> Allowed<T, Read> {
    /// 読み取り認可済みの値を、同じ [`Actor`] で更新認可に昇格します。
    pub fn try_into_update(self) -> Result<Allowed<T, Update>, DomainError>
    where
        T: AuthorizationGuardDefinitions,
    {
        if self.value.can_update(&self.actor) {
            Ok(Allowed::mint(self.value, self.actor))
        } else {
            Err(DomainError::Forbidden)
        }
    }

    /// 読み取り認可済みの値を、同じ [`Actor`] で削除認可に昇格します。
    pub fn try_into_delete(self) -> Result<Allowed<T, Delete>, DomainError>
    where
        T: AuthorizationGuardDefinitions,
    {
        if self.value.can_delete(&self.actor) {
            Ok(Allowed::mint(self.value, self.actor))
        } else {
            Err(DomainError::Forbidden)
        }
    }
}

impl<T: AuthorizationGuardDefinitions> AuthorizationGuard<T, Create> {
    pub(crate) fn new(guard_target: T) -> Self {
        Self {
            guard_target,
            _phantom_data: PhantomData,
        }
    }

    /// [`AuthorizationGuardDefinitions::can_create`] の条件で作成操作を認可します。
    pub fn try_create(self, actor: Actor) -> Result<Allowed<T, Create>, DomainError> {
        if self.guard_target.can_create(&actor) {
            Ok(Allowed::mint(self.guard_target, actor))
        } else {
            Err(DomainError::Forbidden)
        }
    }

    /// [`AuthorizationGuard`] の Action を [`Read`] に変換します。
    pub fn into_read(self) -> AuthorizationGuard<T, Read> {
        AuthorizationGuard {
            guard_target: self.guard_target,
            _phantom_data: PhantomData,
        }
    }

    pub fn into_update(self) -> AuthorizationGuard<T, Update> {
        AuthorizationGuard {
            guard_target: self.guard_target,
            _phantom_data: PhantomData,
        }
    }
}

impl<T: AuthorizationGuardDefinitions> AuthorizationGuard<T, Update> {
    /// [`AuthorizationGuardDefinitions::can_update`] の条件で更新操作を認可します。
    pub fn try_update(self, actor: Actor) -> Result<Allowed<T, Update>, DomainError> {
        if self.guard_target.can_update(&actor) {
            Ok(Allowed::mint(self.guard_target, actor))
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
            _phantom_data: PhantomData,
        }
    }

    /// [`AuthorizationGuard`] の Action を [`Read`] に変換します。
    pub fn into_read(self) -> AuthorizationGuard<T, Read> {
        AuthorizationGuard {
            guard_target: self.guard_target,
            _phantom_data: PhantomData,
        }
    }

    /// [`AuthorizationGuard`] の Action を [`Delete`] に変換します。
    pub fn into_delete(self) -> AuthorizationGuard<T, Delete> {
        AuthorizationGuard {
            guard_target: self.guard_target,
            _phantom_data: PhantomData,
        }
    }
}

impl<T: AuthorizationGuardDefinitions> AuthorizationGuard<T, Read> {
    /// [`AuthorizationGuardDefinitions::can_read`] の条件で読み取り操作を認可します。
    pub fn try_read(self, actor: Actor) -> Result<Allowed<T, Read>, DomainError> {
        if self.guard_target.can_read(&actor) {
            Ok(Allowed::mint(self.guard_target, actor))
        } else {
            Err(DomainError::Forbidden)
        }
    }

    /// [`AuthorizationGuard`] の Action を [`Update`] に変換します。
    pub fn into_update(self) -> AuthorizationGuard<T, Update> {
        AuthorizationGuard {
            guard_target: self.guard_target,
            _phantom_data: PhantomData,
        }
    }

    /// [`AuthorizationGuard`] の Action を [`Delete`] に変換します。
    pub fn into_delete(self) -> AuthorizationGuard<T, Delete> {
        AuthorizationGuard {
            guard_target: self.guard_target,
            _phantom_data: PhantomData,
        }
    }
}

impl<T: AuthorizationGuardDefinitions> AuthorizationGuard<T, Delete> {
    /// [`AuthorizationGuardDefinitions::can_delete`] の条件で削除操作を認可します。
    pub fn try_delete(self, actor: Actor) -> Result<Allowed<T, Delete>, DomainError> {
        if self.guard_target.can_delete(&actor) {
            Ok(Allowed::mint(self.guard_target, actor))
        } else {
            Err(DomainError::Forbidden)
        }
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
            guard_target,
            _phantom_data: PhantomData,
        }
    }
}

impl<T: AuthorizationGuardDefinitions> From<T> for AuthorizationGuard<T, Update> {
    fn from(guard_target: T) -> Self {
        Self {
            guard_target,
            _phantom_data: PhantomData,
        }
    }
}

impl<T: AuthorizationGuardDefinitions> From<T> for AuthorizationGuard<T, Delete> {
    fn from(guard_target: T) -> Self {
        Self {
            guard_target,
            _phantom_data: PhantomData,
        }
    }
}
