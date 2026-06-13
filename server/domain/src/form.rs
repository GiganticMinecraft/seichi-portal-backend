pub mod answer;
pub mod comment;
pub mod label;
pub mod message;
pub mod message_thread;
pub mod models;
pub mod question;
pub mod service;
pub mod settings;

use crate::user::models::{Actor, Role::Administrator, User};

pub(super) fn is_administrator(actor: &Actor) -> bool {
    matches!(actor, Actor::User(User::ActiveUser(user)) if user.role() == &Administrator)
}
