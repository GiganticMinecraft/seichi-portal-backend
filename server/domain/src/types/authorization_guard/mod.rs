mod action;
mod child_guard;
mod proof;
mod role;
mod self_guard;

pub use action::{Actions, Create, Delete, Read, Update};
pub use child_guard::{BelongsTo, GuardedBy, ParentGuarded};
pub use proof::Allowed;
pub use role::AuthorizationRole;
pub use self_guard::{AuthorizationGuard, AuthorizationGuardDefinitions, SelfGuarded};

#[cfg(test)]
mod test {
    use uuid::Uuid;

    use crate::{
        types::authorization_guard::{
            Allowed, AuthorizationGuard, AuthorizationGuardDefinitions, AuthorizationRole, Delete,
            SelfGuarded,
        },
        user::models::{ActiveUser, Actor, Role, User},
    };

    #[derive(Clone, PartialEq, Debug)]
    struct AuthorizationGuardTestStruct {
        pub _value: String,
    }

    impl AuthorizationRole for AuthorizationGuardTestStruct {
        type Role = SelfGuarded;
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
