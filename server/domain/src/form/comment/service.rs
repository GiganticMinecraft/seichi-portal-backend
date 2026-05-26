use crate::{
    form::comment::models::Comment,
    user::models::{Actor, Role::Administrator, User},
};

impl Comment {
    pub fn can_create_on_entry(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::User(User::ActiveUser(_)))
    }

    pub fn can_update_on_entry(&self, actor: &Actor) -> bool {
        match actor {
            Actor::User(User::ActiveUser(actor)) => {
                self.commented_by() == actor.id() || actor.role() == &Administrator
            }
            _ => false,
        }
    }

    pub fn can_delete_on_entry(&self, actor: &Actor) -> bool {
        match actor {
            Actor::User(User::ActiveUser(actor)) => {
                self.commented_by() == actor.id() || actor.role() == &Administrator
            }
            _ => false,
        }
    }
}
