use chrono::{DateTime, Utc};
use domain::types::authorization_guard::{Allowed, AuthorizationGuard, Create, Read, Update};
use domain::{
    account::models::AccountUser,
    auth::Actor,
    notification::models::{
        Notification, NotificationId, NotificationPagePosition, NotificationPreference,
    },
    pagination::{Page, PageRequest},
    repository::{
        notification_repository::NotificationRepository, user_repository::UserRepository,
    },
};
use errors::{Error, usecase::UseCaseError};
use uuid::Uuid;

pub struct NotificationUseCase<
    'a,
    NotificationRepo: NotificationRepository,
    UserRepo: UserRepository,
> {
    pub repository: &'a NotificationRepo,
    pub user_repository: &'a UserRepo,
}

fn mark_notifications_as_read(
    notifications: Vec<AuthorizationGuard<Notification, Read>>,
    actor: &Actor,
    read_at: DateTime<Utc>,
) -> Result<Vec<Allowed<Notification, Update>>, Error> {
    notifications
        .into_iter()
        .map(|notification| {
            let notification = notification.try_read(actor.clone()).map_err(Error::from)?;
            let notification = notification.try_into_update().map_err(Error::from)?;
            Ok(notification.map(|notification| notification.mark_as_read(read_at)))
        })
        .collect()
}

impl<R1: NotificationRepository, R2: UserRepository> NotificationUseCase<'_, R1, R2> {
    pub async fn fetch_notifications(
        &self,
        actor: &AccountUser,
        request: PageRequest<NotificationPagePosition>,
    ) -> Result<Page<Notification, NotificationPagePosition>, Error> {
        let actor_user = Actor::from(actor.clone());
        let page = self
            .repository
            .fetch_notifications(*actor.id(), request)
            .await?;
        let (notifications, next) = page.into_parts();
        let notifications = notifications
            .into_iter()
            .map(|notification| {
                notification
                    .try_read(actor_user.clone())
                    .map(|notification| notification.into_inner())
                    .map_err(Into::into)
            })
            .collect::<Result<Vec<_>, Error>>()?;

        Ok(Page::new(notifications, next))
    }

    pub async fn mark_notification_as_read(
        &self,
        actor: &AccountUser,
        notification_id: NotificationId,
    ) -> Result<(), Error> {
        let notification = self
            .repository
            .fetch_notification(notification_id)
            .await?
            .ok_or(UseCaseError::NotificationNotFound)?;
        let notification = notification
            .into_update()
            .map(|notification| notification.mark_as_read(Utc::now()))
            .try_update(Actor::from(actor.clone()))?;

        self.repository.update_notification(notification).await
    }

    pub async fn mark_all_notifications_as_read(&self, actor: &AccountUser) -> Result<(), Error> {
        let recipient_id = *actor.id();
        let actor = Actor::from(actor.clone());
        let read_at = Utc::now();
        let notifications = self
            .repository
            .fetch_all_notifications(recipient_id)
            .await?;
        let notifications = mark_notifications_as_read(notifications, &actor, read_at)?;

        self.repository.update_notifications(notifications).await
    }

    pub async fn fetch_notification_settings(
        &self,
        actor: AccountUser,
        target: Uuid,
    ) -> Result<NotificationPreference, Error> {
        let actor_user = Actor::from(actor);
        let notification_settings = self.repository.fetch_notification_settings(target).await?;

        match notification_settings {
            Some(notification_settings) => notification_settings
                .try_read(actor_user.clone())
                .map(|settings| settings.into_inner())
                .map_err(Into::into),
            None => {
                let target_user = self
                    .user_repository
                    .find_by(target)
                    .await?
                    .ok_or(Error::from(UseCaseError::UserNotFound))?;

                let target_user = target_user.try_read(actor_user.clone())?.into_inner();
                let notification_settings: AuthorizationGuard<NotificationPreference, Create> =
                    NotificationPreference::new(*target_user.id()).into();

                Ok(notification_settings
                    .into_read()
                    .try_read(actor_user.clone())?
                    .into_inner())
            }
        }
    }

    pub async fn update_notification_settings(
        &self,
        actor: &AccountUser,
        is_send_message_notification: Option<bool>,
    ) -> Result<(), Error> {
        // NOTE: Discord への通知設定は、Discord への連携がすでに行われていなければならない
        let user = self
            .user_repository
            .find_by(actor.id().into_inner())
            .await?
            .ok_or(UseCaseError::UserNotFound)?
            .try_read(Actor::from(actor.clone()))?;

        let discord_user = self.user_repository.fetch_discord_user(&user).await?;

        if discord_user.is_none() {
            return Err(Error::from(UseCaseError::DiscordNotLinked));
        }

        let current_settings = self
            .repository
            .fetch_notification_settings(actor.id().into_inner())
            .await?;

        let current_settings = match current_settings {
            Some(settings) => settings,
            None => {
                let preference = NotificationPreference::new(*actor.id());

                self.repository
                    .create_notification_settings(
                        AuthorizationGuard::<_, Create>::from(preference.clone())
                            .try_create(Actor::from(actor.clone()))?,
                    )
                    .await?;

                AuthorizationGuard::<_, Read>::from(preference)
            }
        };

        match is_send_message_notification {
            Some(is_send_message_notification) => {
                let updated_notification_settings = current_settings
                    .into_update()
                    .map(|settings| {
                        settings.update_send_message_notification(is_send_message_notification)
                    })
                    .try_update(Actor::from(actor.clone()))?;

                self.repository
                    .update_notification_settings(updated_notification_settings)
                    .await
            }
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::{
        account::models::{Role, UserId},
        form::answer::AnswerId,
        notification::models::{NotificationContent, NotificationType},
        pagination::PageLimit,
        repository::notification_repository::NotificationRepository,
        types::authorization_guard::AuthorizationGuard,
    };
    use errors::domain::DomainError;
    use uuid::Uuid;

    use crate::test_utils::repositories::{
        FormUseCaseTestRepositories, InMemoryNotificationRepository,
    };

    fn user(name: &str, role: Role) -> AccountUser {
        AccountUser::new(name.to_string(), UserId::from(Uuid::new_v4()), role)
    }

    fn notification(recipient_id: UserId) -> Notification {
        Notification::new(
            recipient_id,
            NotificationType::MessageReceived,
            AnswerId::new(),
            NotificationContent::new("title", "body", "https://example.com"),
            Utc::now(),
        )
    }

    async fn create_notification(
        repository: &InMemoryNotificationRepository,
        notification: Notification,
    ) {
        repository
            .create_notification(
                AuthorizationGuard::from(notification)
                    .try_create(Actor::System)
                    .unwrap(),
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn notifications_are_scoped_to_the_owner_and_read_all_updates_only_the_owner() {
        let owner = user("owner", Role::StandardUser);
        let other_user = user("other", Role::StandardUser);
        let administrator = user("administrator", Role::Administrator);
        let repositories = FormUseCaseTestRepositories::default();

        let owner_read = notification(*owner.id()).mark_as_read(Utc::now());
        let owner_read_id = *owner_read.id();
        let owner_read_at = (*owner_read.read_at()).expect("read notification has a timestamp");
        let owner_unread = notification(*owner.id());
        let other_unread = notification(*other_user.id());
        let other_unread_id = *other_unread.id();
        let administrator_unread = notification(*administrator.id());

        create_notification(&repositories.notification_repository, owner_read).await;
        create_notification(&repositories.notification_repository, owner_unread).await;
        for _ in 0..50 {
            create_notification(
                &repositories.notification_repository,
                notification(*owner.id()),
            )
            .await;
        }
        create_notification(&repositories.notification_repository, other_unread).await;
        create_notification(&repositories.notification_repository, administrator_unread).await;

        let usecase = NotificationUseCase {
            repository: &repositories.notification_repository,
            user_repository: &repositories.user_repository,
        };
        let request = PageRequest::first(PageLimit::default_limit());

        let owner_notifications = usecase
            .fetch_notifications(&owner, request.clone())
            .await
            .unwrap();
        assert_eq!(owner_notifications.items().len(), 50);
        assert!(owner_notifications.next().is_some());
        assert!(
            owner_notifications
                .items()
                .iter()
                .all(|notification| *notification.recipient_id() == *owner.id())
        );

        assert_eq!(
            usecase
                .mark_notification_as_read(&owner, other_unread_id)
                .await,
            Err(Error::from(DomainError::Forbidden))
        );

        usecase
            .mark_all_notifications_as_read(&owner)
            .await
            .unwrap();

        let owner_notifications = usecase
            .fetch_notifications(&owner, PageRequest::first(PageLimit::try_new(100).unwrap()))
            .await
            .unwrap();
        assert_eq!(owner_notifications.items().len(), 52);
        assert_eq!(owner_notifications.next(), None);
        assert!(
            owner_notifications
                .items()
                .iter()
                .all(|notification| notification.read_at().is_some())
        );
        assert_eq!(
            *owner_notifications
                .items()
                .iter()
                .find(|notification| *notification.id() == owner_read_id)
                .expect("existing read notification is present")
                .read_at(),
            Some(owner_read_at)
        );

        let other_notifications = usecase
            .fetch_notifications(&other_user, request.clone())
            .await
            .unwrap();
        assert_eq!(other_notifications.items().len(), 1);
        assert!(other_notifications.items()[0].read_at().is_none());

        let administrator_notifications = usecase
            .fetch_notifications(&administrator, request.clone())
            .await
            .unwrap();
        assert_eq!(administrator_notifications.items().len(), 1);
        assert!(administrator_notifications.items()[0].read_at().is_none());

        usecase
            .mark_all_notifications_as_read(&administrator)
            .await
            .unwrap();

        let administrator_notifications = usecase
            .fetch_notifications(&administrator, request)
            .await
            .unwrap();
        assert!(administrator_notifications.items()[0].read_at().is_some());
        assert!(
            usecase
                .fetch_notifications(&other_user, PageRequest::first(PageLimit::default_limit()))
                .await
                .unwrap()
                .items()[0]
                .read_at()
                .is_none()
        );
    }

    #[tokio::test]
    async fn read_all_succeeds_when_the_actor_has_no_notifications() {
        let actor = user("actor", Role::StandardUser);
        let repositories = FormUseCaseTestRepositories::default();
        let usecase = NotificationUseCase {
            repository: &repositories.notification_repository,
            user_repository: &repositories.user_repository,
        };

        usecase
            .mark_all_notifications_as_read(&actor)
            .await
            .unwrap();

        assert!(
            usecase
                .fetch_notifications(&actor, PageRequest::first(PageLimit::default_limit()))
                .await
                .unwrap()
                .items()
                .is_empty()
        );
    }
}
