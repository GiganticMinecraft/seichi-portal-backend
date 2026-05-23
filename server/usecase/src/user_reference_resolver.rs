use std::collections::HashMap;

use domain::{
    repository::user_repository::UserRepository,
    user::models::{User, UserId},
};
use errors::Error;

pub(crate) async fn resolve_user_references<R: UserRepository + ?Sized>(
    repo: &R,
    actor: &User,
    user_ids: Vec<UserId>,
) -> Result<HashMap<UserId, User>, Error> {
    if user_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let uuids = user_ids
        .into_iter()
        .map(UserId::into_inner)
        .collect::<Vec<_>>();

    repo.find_by_ids(uuids)
        .await?
        .into_iter()
        .map(|guard| {
            let user = guard.try_into_read(actor)?;
            Ok((user.id, user))
        })
        .collect()
}
