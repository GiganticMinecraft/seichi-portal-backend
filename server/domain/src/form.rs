pub mod answer;
pub mod comment;
pub mod label;
pub mod message;
pub mod message_thread;
pub mod models;
pub mod question;
pub mod service;
pub mod settings;

use crate::{account::models::Role::Administrator, auth::Actor};

pub(super) fn is_administrator(actor: &Actor) -> bool {
    matches!(actor, Actor::AccountUser(user) if user.role() == &Administrator)
}
