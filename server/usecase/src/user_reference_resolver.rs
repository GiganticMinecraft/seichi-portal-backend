use std::collections::HashMap;

use domain::{
    account::models::{AccountUser, UserId},
    auth::Actor,
    repository::user_repository::UserRepository,
};
use errors::Error;

pub(crate) async fn resolve_user_references<R: UserRepository + ?Sized>(
    repo: &R,
    actor: &AccountUser,
    user_ids: Vec<UserId>,
) -> Result<HashMap<UserId, AccountUser>, Error> {
    if user_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let actor_user = Actor::from(actor.clone());
    let uuids = user_ids
        .into_iter()
        .map(UserId::into_inner)
        .collect::<Vec<_>>();

    repo.find_by_ids(uuids)
        .await?
        .into_iter()
        .map(|guard| {
            let user = guard.try_read(actor_user.clone())?.into_inner();
            Ok((*user.id(), user))
        })
        .collect()
}
