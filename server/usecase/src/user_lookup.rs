use std::collections::HashMap;

use domain::{
    repository::user_repository::UserRepository,
    user::models::{User, UserId},
};
use errors::{Error, usecase::UseCaseError::UserNotFound};

pub(crate) async fn find_user<R: UserRepository + ?Sized>(
    repo: &R,
    actor: &User,
    user_id: UserId,
) -> Result<User, Error> {
    repo.find_by(user_id.into_inner())
        .await?
        .ok_or(Error::from(UserNotFound))?
        .try_into_read(actor)
        .map_err(Into::into)
}

pub(crate) async fn find_users<R: UserRepository + ?Sized>(
    repo: &R,
    actor: &User,
    user_ids: Vec<UserId>,
) -> Result<HashMap<UserId, User>, Error> {
    if user_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let uuids = user_ids
        .into_iter()
        .map(|id| id.into_inner())
        .collect::<Vec<_>>();

    let guards = repo.find_by_ids(uuids).await?;

    let mut map = HashMap::with_capacity(guards.len());
    for guard in guards {
        let user = guard.try_into_read(actor)?;
        map.insert(user.id, user);
    }
    Ok(map)
}
