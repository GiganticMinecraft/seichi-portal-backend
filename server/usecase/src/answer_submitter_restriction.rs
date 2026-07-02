use chrono::{DateTime, Utc};
use domain::{
    account::models::AccountUser,
    auth::Actor,
    form::answer::{AnswerSubmitterRestriction, AnswerSubmitterRestrictionReason},
    repository::{
        answer_submitter_restriction_repository::AnswerSubmitterRestrictionRepository,
        user_repository::UserRepository,
    },
    types::authorization_guard::{AuthorizationGuard, Create},
};
use errors::{Error, usecase::UseCaseError};
use uuid::Uuid;

pub struct AnswerSubmitterRestrictionUseCase<
    'a,
    UserRepo: UserRepository,
    RestrictionRepo: AnswerSubmitterRestrictionRepository,
> {
    pub user_repository: &'a UserRepo,
    pub restriction_repository: &'a RestrictionRepo,
}

impl<R1: UserRepository, R2: AnswerSubmitterRestrictionRepository>
    AnswerSubmitterRestrictionUseCase<'_, R1, R2>
{
    pub async fn restrict(
        &self,
        actor: &AccountUser,
        submitter_id: Uuid,
        reason: AnswerSubmitterRestrictionReason,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<AnswerSubmitterRestriction, Error> {
        let actor_ref = Actor::from(actor.clone());
        self.user_repository
            .find_by(submitter_id)
            .await?
            .ok_or(Error::from(UseCaseError::UserNotFound))?;

        let restriction = AnswerSubmitterRestriction::new(
            submitter_id.into(),
            reason,
            *actor.id(),
            Utc::now(),
            expires_at,
        )?;

        self.restriction_repository
            .restrict(
                AuthorizationGuard::<_, Create>::from(restriction.clone())
                    .try_create(actor_ref.clone())?,
            )
            .await?;

        Ok(restriction)
    }

    pub async fn lift(&self, actor: &AccountUser, submitter_id: Uuid) -> Result<(), Error> {
        let actor_ref = Actor::from(actor.clone());

        self.user_repository
            .find_by(submitter_id)
            .await?
            .ok_or(Error::from(UseCaseError::UserNotFound))?;

        let Some(restriction) = self
            .restriction_repository
            .fetch_active_by_submitter_id(submitter_id)
            .await?
        else {
            return Ok(());
        };

        self.restriction_repository
            .lift(restriction.into_delete().try_delete(actor_ref)?)
            .await
    }

    pub async fn fetch_active(
        &self,
        actor: &AccountUser,
        submitter_id: Uuid,
    ) -> Result<Option<AnswerSubmitterRestriction>, Error> {
        let actor_ref = Actor::from(actor.clone());
        self.user_repository
            .find_by(submitter_id)
            .await?
            .ok_or(Error::from(UseCaseError::UserNotFound))?;

        self.restriction_repository
            .fetch_active_by_submitter_id(submitter_id)
            .await?
            .map(|restriction| {
                restriction
                    .try_read(actor_ref.clone())
                    .map(|restriction| restriction.into_inner())
            })
            .transpose()
            .map_err(Into::into)
    }

    pub async fn list_history(
        &self,
        actor: &AccountUser,
        submitter_id: Uuid,
    ) -> Result<Vec<AnswerSubmitterRestriction>, Error> {
        let actor_ref = Actor::from(actor.clone());
        self.user_repository
            .find_by(submitter_id)
            .await?
            .ok_or(Error::from(UseCaseError::UserNotFound))?;

        self.restriction_repository
            .list_by_submitter_id(submitter_id)
            .await?
            .try_read(actor_ref)
            .map(|history| history.into_inner().into_restrictions())
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use domain::account::models::{Role, UserId};
    use errors::domain::DomainError;

    use super::*;
    use crate::test_utils::repositories::FormUseCaseTestRepositories;

    fn user(seed: u128, name: &str, role: Role) -> AccountUser {
        AccountUser::new(name.to_string(), UserId::from(Uuid::from_u128(seed)), role)
    }

    #[tokio::test]
    async fn list_history_rejects_other_standard_user_even_when_history_is_empty() {
        let actor = user(1, "actor", Role::StandardUser);
        let submitter = user(2, "submitter", Role::StandardUser);
        let repositories = FormUseCaseTestRepositories::default();
        repositories.user_repository.save_user(submitter.clone());
        let usecase = AnswerSubmitterRestrictionUseCase {
            user_repository: &repositories.user_repository,
            restriction_repository: &repositories.answer_submitter_restriction_repository,
        };

        let result = usecase
            .list_history(&actor, submitter.id().into_inner())
            .await;

        assert_eq!(result, Err(Error::from(DomainError::Forbidden)));
    }

    #[tokio::test]
    async fn list_history_allows_submitter_to_read_empty_history() {
        let submitter = user(1, "submitter", Role::StandardUser);
        let repositories = FormUseCaseTestRepositories::default();
        repositories.user_repository.save_user(submitter.clone());
        let usecase = AnswerSubmitterRestrictionUseCase {
            user_repository: &repositories.user_repository,
            restriction_repository: &repositories.answer_submitter_restriction_repository,
        };

        let result = usecase
            .list_history(&submitter, submitter.id().into_inner())
            .await;

        assert_eq!(result, Ok(vec![]));
    }
}
