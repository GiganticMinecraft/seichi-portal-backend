use domain::{
    account::models::AccountUser, auth::Actor, minecraft_ban::MinecraftBan,
    repository::minecraft_ban_repository::MinecraftBanRepository,
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
        let user_id = user_id.into();

        self.repository
            .list_by_user_id(user_id)
            .await?
            .try_read(Actor::from(actor.clone()))
            .map(|history| history.into_inner().into_minecraft_bans())
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use domain::{
        account::models::{Role, UserId},
        minecraft_ban::MinecraftBanHistory,
        repository::minecraft_ban_repository::MinecraftBanRepository,
        types::authorization_guard::{AuthorizationGuard, Read},
    };
    use errors::domain::DomainError;

    use super::*;

    struct EmptyMinecraftBanRepository;

    #[async_trait]
    impl MinecraftBanRepository for EmptyMinecraftBanRepository {
        async fn list_by_user_id(
            &self,
            user_id: UserId,
        ) -> Result<AuthorizationGuard<MinecraftBanHistory, Read>, Error> {
            Ok(MinecraftBanHistory::new(user_id, vec![])?.into())
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
        let repository = EmptyMinecraftBanRepository;
        let usecase = MinecraftBanUseCase {
            repository: &repository,
        };

        let result = usecase.list(&actor, actor.id().into_inner()).await;

        assert_eq!(result, Ok(vec![]));
    }

    #[tokio::test]
    async fn administrator_can_list_an_empty_history() {
        let actor = user(1, Role::Administrator);
        let repository = EmptyMinecraftBanRepository;
        let usecase = MinecraftBanUseCase {
            repository: &repository,
        };

        let result = usecase.list(&actor, Uuid::from_u128(2)).await;

        assert_eq!(result, Ok(vec![]));
    }

    #[tokio::test]
    async fn another_standard_user_is_forbidden() {
        let actor = user(1, Role::StandardUser);
        let repository = EmptyMinecraftBanRepository;
        let usecase = MinecraftBanUseCase {
            repository: &repository,
        };

        let result = usecase.list(&actor, Uuid::from_u128(2)).await;

        assert_eq!(result, Err(Error::from(DomainError::Forbidden)));
    }
}
