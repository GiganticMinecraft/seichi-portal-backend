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
        let history = AuthorizationGuard::<_, Read>::from(MinecraftBanHistory::new(user_id.into()))
            .try_read(Actor::from(actor.clone()))?;

        self.repository.list(history).await
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use async_trait::async_trait;
    use domain::{
        account::models::{Role, UserId},
        minecraft_ban::{MinecraftBan, MinecraftBanHistory},
        repository::minecraft_ban_repository::MinecraftBanRepository,
        types::authorization_guard::{Allowed, Read},
    };
    use errors::domain::DomainError;

    use super::*;

    #[derive(Default)]
    struct RecordingMinecraftBanRepository {
        called: AtomicBool,
    }

    #[async_trait]
    impl MinecraftBanRepository for RecordingMinecraftBanRepository {
        async fn list(
            &self,
            _history: Allowed<MinecraftBanHistory, Read>,
        ) -> Result<Vec<MinecraftBan>, Error> {
            self.called.store(true, Ordering::SeqCst);
            Ok(vec![])
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
    async fn another_standard_user_is_forbidden_before_repository_is_called() {
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
