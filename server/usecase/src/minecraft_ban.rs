use domain::{
    account::models::AccountUser,
    auth::Actor,
    minecraft_ban::{MinecraftBan, MinecraftBanHistory},
    repository::minecraft_ban_repository::MinecraftBanRepository,
    types::authorization_guard::{AuthorizationGuard, Read},
};
use errors::Error;
use uuid::Uuid;

pub struct MinecraftBanUseCase<'a, Repo: MinecraftBanRepository> {
    pub repository: &'a Repo,
}

impl<Repo: MinecraftBanRepository> MinecraftBanUseCase<'_, Repo> {
    pub async fn list(
        &self,
        actor: &AccountUser,
        user_id: Uuid,
    ) -> Result<Vec<MinecraftBan>, Error> {
        let actor = Actor::from(actor.clone());
        let history =
            AuthorizationGuard::<_, Read>::from(MinecraftBanHistory::new(user_id.into(), vec![])?)
                .try_read(actor.clone())?;

        self.repository
            .list_by_user_id(history)
            .await?
            .try_read(actor)
            .map(|history| history.into_inner().into_minecraft_bans())
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use async_trait::async_trait;
    use domain::{
        account::models::{Role, UserId},
        minecraft_ban::MinecraftBanHistory,
        repository::minecraft_ban_repository::MinecraftBanRepository,
        types::authorization_guard::{Allowed, AuthorizationGuard, Read},
    };
    use errors::domain::DomainError;

    use super::*;

    #[derive(Default)]
    struct RecordingMinecraftBanRepository {
        called: AtomicBool,
    }

    #[async_trait]
    impl MinecraftBanRepository for RecordingMinecraftBanRepository {
        async fn list_by_user_id(
            &self,
            history: Allowed<MinecraftBanHistory, Read>,
        ) -> Result<AuthorizationGuard<MinecraftBanHistory, Read>, Error> {
            self.called.store(true, Ordering::SeqCst);
            Ok(MinecraftBanHistory::new(history.user_id(), vec![])?.into())
        }
    }

    fn user(seed: u128, role: Role) -> AccountUser {
        AccountUser::new(
            "user".to_string(),
            UserId::from(Uuid::from_u128(seed)),
            role,
        )
    }

    #[tokio::test]
    async fn owner_can_list_an_empty_history() {
        let actor = user(1, Role::StandardUser);
        let repository = RecordingMinecraftBanRepository::default();
        let usecase = MinecraftBanUseCase {
            repository: &repository,
        };

        let result = usecase.list(&actor, actor.id().into_inner()).await;

        assert_eq!(result, Ok(vec![]));
        assert!(repository.called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn administrator_can_list_an_empty_history() {
        let actor = user(1, Role::Administrator);
        let repository = RecordingMinecraftBanRepository::default();
        let usecase = MinecraftBanUseCase {
            repository: &repository,
        };

        let result = usecase.list(&actor, Uuid::from_u128(2)).await;

        assert_eq!(result, Ok(vec![]));
        assert!(repository.called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn another_standard_user_is_forbidden() {
        let actor = user(1, Role::StandardUser);
        let repository = RecordingMinecraftBanRepository::default();
        let usecase = MinecraftBanUseCase {
            repository: &repository,
        };

        let result = usecase.list(&actor, Uuid::from_u128(2)).await;

        assert_eq!(result, Err(Error::from(DomainError::Forbidden)));
        assert!(!repository.called.load(Ordering::SeqCst));
    }
}
