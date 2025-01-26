use std::future::Future;

use errors::{domain::DomainError, Error};

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

/// [`AuthorizationGuardWithContext`] は、[`User`] と [`Context`] を受け取り、
/// [`guard_target`] に対して CRUD 操作が可能であるかを定義するための抽象です。
#[derive(Debug)]
pub struct AuthorizationGuardWithContext<
    T: AuthorizationGuardWithContextDefinitions<T, Context>,
    A: Actions,
    Context,
> {
    guard_target: T,
    _phantom_data: std::marker::PhantomData<(A, Context)>,
}

// NOTE: 実装時点(2024/10/27)では、AuthorizationGuardWithContext の Action は以下のようにのみ変換することができます
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
//  AuthorizationGuardWithContext<T, Delete> から誤って `.into_read()` 関数を呼び出すことで
//  Read 権限を持つユーザーによってデータが削除されるという事故が発生する可能性があります。
//  このような事故を防ぐために、AuthorizationGuardWithContext の Action の変換は上記のように限定されています。
impl<T: AuthorizationGuardWithContextDefinitions<T, Context>, Context>
    AuthorizationGuardWithContext<T, Create, Context>
{
    pub fn new(guard_target: T) -> Self {
        Self {
            guard_target,
            _phantom_data: std::marker::PhantomData,
        }
    }

    /// [`AuthorizationGuardWithContextDefinitions::can_create`] の条件で作成操作 `f` を試みます。
    pub fn try_create<'a, R, F>(
        &'a self,
        actor: &User,
        f: F,
        context: &Context,
    ) -> Result<R, DomainError>
    where
        F: FnOnce(&'a T) -> R,
    {
        if self.guard_target.can_create(actor, context) {
            Ok(f(&self.guard_target))
        } else {
            Err(DomainError::Forbidden)
        }
    }

    /// [`AuthorizationGuardWithContext`] の Action を [`Read`] に変換します。
    pub fn into_read(self) -> AuthorizationGuardWithContext<T, Read, Context> {
        AuthorizationGuardWithContext {
            guard_target: self.guard_target,
            _phantom_data: std::marker::PhantomData,
        }
    }

    pub fn into_update(self) -> AuthorizationGuardWithContext<T, Update, Context> {
        AuthorizationGuardWithContext {
            guard_target: self.guard_target,
            _phantom_data: std::marker::PhantomData,
        }
    }
}

impl<T: AuthorizationGuardWithContextDefinitions<T, Context>, Context>
    AuthorizationGuardWithContext<T, Update, Context>
{
    /// [`AuthorizationGuardWithContextDefinitions::can_update`] の条件で更新操作 `f` を試みます。
    pub fn try_update<'a, R, F>(
        &'a self,
        actor: &User,
        f: F,
        context: &Context,
    ) -> Result<R, DomainError>
    where
        F: FnOnce(&'a T) -> R,
    {
        if self.guard_target.can_update(actor, context) {
            Ok(f(&self.guard_target))
        } else {
            Err(DomainError::Forbidden)
        }
    }

    /// [`T`] の値に対して map 相当の操作を行います。
    pub fn map<F>(self, f: F) -> AuthorizationGuardWithContext<T, Update, Context>
    where
        F: FnOnce(T) -> T,
    {
        AuthorizationGuardWithContext {
            guard_target: f(self.guard_target),
            _phantom_data: std::marker::PhantomData,
        }
    }

    /// [`AuthorizationGuardWithContext`] の Action を [`Read`] に変換します。
    pub fn into_read(self) -> AuthorizationGuardWithContext<T, Read, Context> {
        AuthorizationGuardWithContext {
            guard_target: self.guard_target,
            _phantom_data: std::marker::PhantomData,
        }
    }

    /// [`AuthorizationGuardWithContext`] の Action を [`Delete`] に変換します。
    pub fn into_delete(self) -> AuthorizationGuardWithContext<T, Delete, Context> {
        AuthorizationGuardWithContext {
            guard_target: self.guard_target,
            _phantom_data: std::marker::PhantomData,
        }
    }
}

impl<T: AuthorizationGuardWithContextDefinitions<T, Context>, Context>
    AuthorizationGuardWithContext<T, Read, Context>
{
    /// `actor` が `guard_target` の参照を取得することを試みます。
    pub fn try_read(&self, actor: &User, context: &Context) -> Result<&T, DomainError> {
        if self.guard_target.can_read(actor, context) {
            Ok(&self.guard_target)
        } else {
            Err(DomainError::Forbidden)
        }
    }

    /// `actor` が `guard_target` を取得することを試みます。
    pub fn try_into_read(self, actor: &User, context: &Context) -> Result<T, DomainError> {
        if self.guard_target.can_read(actor, context) {
            Ok(self.guard_target)
        } else {
            Err(DomainError::Forbidden)
        }
    }

    pub async fn try_into_read_with_context_fn<Fut>(
        self,
        actor: &User,
        context_fn: impl FnOnce(&T) -> Fut,
    ) -> Result<T, Error>
    where
        Fut: Future<Output = Result<Context, Error>> + Sized,
    {
        if self
            .guard_target
            .can_read(actor, &context_fn(&self.guard_target).await?)
        {
            Ok(self.guard_target)
        } else {
            Err(Error::from(DomainError::Forbidden))
        }
    }

    /// [`AuthorizationGuardWithContext`] の Action を [`Update`] に変換します。
    pub fn into_update(self) -> AuthorizationGuardWithContext<T, Update, Context> {
        AuthorizationGuardWithContext {
            guard_target: self.guard_target,
            _phantom_data: std::marker::PhantomData,
        }
    }

    /// [`AuthorizationGuardWithContext`] の Action を [`Delete`] に変換します。
    pub fn into_delete(self) -> AuthorizationGuardWithContext<T, Delete, Context> {
        AuthorizationGuardWithContext {
            guard_target: self.guard_target,
            _phantom_data: std::marker::PhantomData,
        }
    }
}

impl<T: AuthorizationGuardWithContextDefinitions<T, Context>, Context>
    AuthorizationGuardWithContext<T, Delete, Context>
{
    /// [`AuthorizationGuardWithContextDefinitions::can_delete`] の条件で削除操作 `f` を試みます。
    pub fn try_delete<'a, R, F>(
        &'a self,
        actor: &User,
        f: F,
        context: &Context,
    ) -> Result<R, DomainError>
    where
        F: FnOnce(&'a T) -> R,
    {
        if self.guard_target.can_delete(actor, context) {
            Ok(f(&self.guard_target))
        } else {
            Err(DomainError::Forbidden)
        }
    }

    /// [`AuthorizationGuardWithContextDefinitions::can_delete`] の条件で削除操作 `f` を試みます。
    /// この関数は、`guard_target` を所有権を持つ形で操作を行います。
    pub fn try_into_delete<R, F>(
        self,
        actor: &User,
        f: F,
        context: &Context,
    ) -> Result<R, DomainError>
    where
        F: FnOnce(T) -> R,
    {
        if self.guard_target.can_delete(actor, context) {
            Ok(f(self.guard_target))
        } else {
            Err(DomainError::Forbidden)
        }
    }
}

impl<T: AuthorizationGuardWithContextDefinitions<T, Context>, Action: Actions, Context>
    AuthorizationGuardWithContext<T, Action, Context>
{
    pub async fn create_context<Fut>(
        &self,
        context_fn: impl FnOnce(&T) -> Fut,
    ) -> Result<Context, Error>
    where
        Fut: Future<Output = Result<Context, Error>> + Sized,
    {
        context_fn(&self.guard_target).await
    }
}

pub trait AuthorizationGuardWithContextDefinitions<T, Context> {
    fn can_create(&self, actor: &User, context: &Context) -> bool;
    fn can_read(&self, actor: &User, context: &Context) -> bool;
    fn can_update(&self, actor: &User, context: &Context) -> bool;
    fn can_delete(&self, actor: &User, context: &Context) -> bool;
}

#[cfg(test)]
mod test {
    use uuid::Uuid;

    use crate::{
        types::authorization_guard_with_context::{
            AuthorizationGuardWithContext, AuthorizationGuardWithContextDefinitions, Create,
        },
        user::models::{Role, User},
    };

    #[derive(Clone, PartialEq, Debug)]
    struct AuthorizationGuardWithContextTestStruct {
        pub _value: String,
    }

    #[derive(Clone, Debug)]
    struct Context {}

    impl AuthorizationGuardWithContextDefinitions<AuthorizationGuardWithContextTestStruct, Context>
        for AuthorizationGuardWithContextTestStruct
    {
        fn can_create(&self, actor: &User, _context: &Context) -> bool {
            actor.role == Role::Administrator
        }

        fn can_read(&self, actor: &User, _context: &Context) -> bool {
            actor.role == Role::Administrator || actor.role == Role::StandardUser
        }

        fn can_update(&self, actor: &User, _context: &Context) -> bool {
            actor.role == Role::Administrator
        }

        fn can_delete(&self, actor: &User, _context: &Context) -> bool {
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

        let context = Context {};

        let guard = AuthorizationGuardWithContext::new(AuthorizationGuardWithContextTestStruct {
            _value: "test".to_string(),
        });

        assert!(&guard.try_create(&admin, |_| {}, &context).is_ok());
        assert!(&guard.try_create(&standard_user, |_| {}, &context).is_err());

        let guard = guard.into_read();
        assert!(&guard.try_read(&admin, &context).is_ok());
        assert!(&guard.try_read(&standard_user, &context).is_ok());

        let guard = guard.into_update();
        assert!(&guard.try_update(&admin, |_| {}, &context).is_ok());
        assert!(&guard.try_update(&standard_user, |_| {}, &context).is_err());

        let guard = guard.into_delete();
        assert!(&guard.try_delete(&admin, |_| {}, &context).is_ok());
        assert!(&guard.try_delete(&standard_user, |_| {}, &context).is_err());
    }

    #[test]
    fn verify_same_data_for_try_read_and_try_into_read() {
        let user = User {
            name: "user".to_string(),
            id: Uuid::new_v4(),
            role: Role::Administrator,
        };

        let context = Context {};

        let guard: AuthorizationGuardWithContext<
            AuthorizationGuardWithContextTestStruct,
            Create,
            Context,
        > = AuthorizationGuardWithContext::new(AuthorizationGuardWithContextTestStruct {
            _value: "test".to_string(),
        });

        let read_guard = guard.into_read();

        let from_into_read = read_guard.try_read(&user, &context);
        assert!(from_into_read.is_ok());

        let from_into_read = from_into_read.unwrap().to_owned();

        let read_into = read_guard.try_into_read(&user, &context);

        assert!(read_into.is_ok());

        assert_eq!(from_into_read, read_into.unwrap());
    }
}
