use errors::domain::DomainError;
use std::marker::PhantomData;
use std::ops::Deref;

use crate::user::models::Actor;

pub trait Actions: private::Sealed {}

#[derive(Debug, Clone, Copy)]
pub struct Create;
#[derive(Debug, Clone, Copy)]
pub struct Read;
#[derive(Debug, Clone, Copy)]
pub struct Update;
#[derive(Debug, Clone, Copy)]
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

/// 指定した操作について認可済みであることを型で表す値です。
///
/// この型は [`AuthorizationGuard`] の `try_*` メソッドや、認可済みの親要素から
/// 子要素を認可するメソッドだけが作成します。`value` と一緒に認可に使った
/// [`Actor`] を保持するため、親子関係を使った追加の認可判定でも同じ利用者を引き継げます。
#[derive(Debug, Clone, PartialEq)]
pub struct Allowed<T, A: Actions> {
    value: T,
    actor: Actor,
    _phantom_data: PhantomData<A>,
}

impl<T, A: Actions> Allowed<T, A> {
    fn mint(value: T, actor: Actor) -> Self {
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
}

impl<T> Allowed<T, Read> {
    /// 読み取り認可済みの親要素から、子要素の読み取り認可済み値を作ります。
    ///
    /// 親要素が実装する [`AuthorizesRead`] で親子関係や所有者などを確認し、
    /// 成功した場合だけ同じ [`Actor`] の [`Allowed<C, Read>`] を返します。
    pub fn authorize_read<C>(&self, child: C) -> Result<Allowed<C, Read>, DomainError>
    where
        T: AuthorizesRead<C>,
    {
        self.value.check(&self.actor, &child)?;
        Ok(Allowed::mint(child, self.actor.clone()))
    }
}

/// 読み取り認可済みの親要素が、子要素の読み取りも認可できることを表すトレイトです。
///
/// 例えば回答が読める利用者に、その回答に紐づくコメントの読み取りも許可する場合に使います。
pub trait AuthorizesRead<Child> {
    fn check(&self, actor: &Actor, child: &Child) -> Result<(), DomainError>;
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

#[cfg(test)]
mod test {
    use uuid::Uuid;

    use crate::{
        types::authorization_guard::{
            Allowed, AuthorizationGuard, AuthorizationGuardDefinitions, Delete,
        },
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

        assert!(&guard.clone().try_create(admin.clone()).is_ok());
        assert!(&guard.try_create(standard_user.clone()).is_err());

        let guard = AuthorizationGuard::new(AuthorizationGuardTestStruct {
            _value: "test".to_string(),
        })
        .into_read();
        assert!(&guard.clone().try_read(admin.clone()).is_ok());
        assert!(&guard.try_read(standard_user.clone()).is_ok());

        let guard = AuthorizationGuard::new(AuthorizationGuardTestStruct {
            _value: "test".to_string(),
        })
        .into_update();
        assert!(&guard.clone().try_update(admin.clone()).is_ok());
        assert!(&guard.try_update(standard_user.clone()).is_err());

        let guard = AuthorizationGuard::<_, Delete>::from(AuthorizationGuardTestStruct {
            _value: "test".to_string(),
        });
        assert!(&guard.clone().try_delete(admin.clone()).is_ok());
        assert!(&guard.try_delete(standard_user.clone()).is_err());
    }

    #[test]
    fn allowed_can_borrow_and_unwrap_value() {
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

        let from_into_read = read_guard.clone().try_read(user.clone());
        assert!(from_into_read.is_ok());

        let from_into_read = from_into_read.unwrap().into_inner();

        let read_into = read_guard.try_read(user.clone()).map(Allowed::into_inner);

        assert!(read_into.is_ok());

        assert_eq!(from_into_read, read_into.unwrap());
    }
}
