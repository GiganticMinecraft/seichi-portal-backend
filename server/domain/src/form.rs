pub mod answer;
pub mod comment;
pub mod comment_attachment;
pub mod comment_thread;
pub mod label;
pub mod message;
pub mod message_thread;
pub mod models;
pub mod question;
pub mod redmine_import;
pub mod service;
pub mod settings;
pub mod submission_restriction;
pub mod submitter;

pub use submission_restriction::{
    FormSubmissionRestriction, FormSubmissionRestrictionHistory, FormSubmissionRestrictionId,
    FormSubmissionRestrictionReason,
};
pub use submitter::FormSubmitter;

use crate::{account::models::Role::Administrator, auth::Actor};

pub(super) fn is_administrator(actor: &Actor) -> bool {
    matches!(actor, Actor::AccountUser(user) if user.role() == &Administrator)
}
