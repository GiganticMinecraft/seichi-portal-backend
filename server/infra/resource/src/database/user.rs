use std::str::FromStr;

use async_trait::async_trait;
use chrono::Utc;
use domain::user::models::{DiscordUserId, Role, User};
use errors::infra::InfraError;
use redis::{Commands, JsonCommands};
use sha256::digest;
use uuid::Uuid;

use crate::database::{
    components::UserDatabase,
    connection::{
        execute_and_values, query_all, query_one_and_values, redis_connection, ConnectionPool,
    },
};

#[async_trait]
impl UserDatabase for ConnectionPool {
    async fn find_by(&self, uuid: Uuid) -> Result<Option<User>, InfraError> {
        Ok(self
            .read_only_transaction(|txn| {
                Box::pin(async move {
                    let query = query_one_and_values(
                        "SELECT name, role FROM users WHERE id = ?",
                        [uuid.to_string().into()],
                        txn,
                    )
                    .await?;

                    let user = query
                        .map(|rs| {
                            Ok::<User, InfraError>(User {
                                name: rs.try_get("", "name")?,
                                id: uuid,
                                role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                            })
                        })
                        .transpose()?;

                    Ok::<_, InfraError>(user)
                })
            })
            .await?)
    }

    async fn upsert_user(&self, user: &User) -> Result<(), InfraError> {
        let params = [
            user.id.to_string().into(),
            user.name.to_owned().into(),
            user.role.to_string().into(),
        ];

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "INSERT INTO users (id, name, role) VALUES (?, ?, ?)
                        ON DUPLICATE KEY UPDATE
                        name = VALUES(name)",
                    params,
                    txn,
                )
                .await?;

                Ok::<(), InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn patch_user_role(&self, uuid: Uuid, role: Role) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    "UPDATE users SET role = ? WHERE id = ?",
                    [role.to_string().into(), uuid.to_string().into()],
                    txn,
                )
                .await?;

                Ok::<(), InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn fetch_all_users(&self) -> Result<Vec<User>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let query = query_all("SELECT id, name, role FROM users", txn).await?;

                let users = query
                    .into_iter()
                    .map(|rs| {
                        Ok::<User, InfraError>(User {
                            name: rs.try_get("", "name")?,
                            id: Uuid::parse_str(&rs.try_get::<String>("", "id")?)?,
                            role: Role::from_str(&rs.try_get::<String>("", "role")?)?,
                        })
                    })
                    .collect::<Result<Vec<User>, InfraError>>()?;

                Ok::<_, InfraError>(users)
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn start_user_session(
        &self,
        xbox_token: String,
        user: &User,
        expires: i32,
    ) -> Result<String, InfraError> {
        let now = Utc::now().timestamp_millis();
        let session_id = digest(format!("{xbox_token}{now}"));

        let mut redis_connection = redis_connection().await;

        redis_connection.json_set::<&str, &str, _, ()>(&session_id, "$", user)?;

        redis_connection.expire::<&str, ()>(&session_id, expires as i64)?;
        Ok(session_id)
    }

    async fn fetch_user_by_session_id(
        &self,
        session_id: String,
    ) -> Result<Option<User>, InfraError> {
        let mut redis_connection = redis_connection().await;

        let user = serde_json::from_str::<Vec<User>>(
            &redis_connection.json_get::<&String, &str, String>(&session_id, "$")?,
        )
        .map(|users| users.into_iter().nth(0))
        .ok()
        .flatten();

        Ok(user)
    }

    async fn end_user_session(&self, session_id: String) -> Result<(), InfraError> {
        let mut redis_connection = redis_connection().await;

        redis_connection.del::<&str, ()>(&session_id)?;

        Ok(())
    }

    async fn link_discord_user(
        &self,
        discord_user_id: &DiscordUserId,
        user: &User,
    ) -> Result<(), InfraError> {
        let user_id = user.id.to_string();
        let discord_user_id = discord_user_id.to_owned().into_inner();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                execute_and_values(
                    r#"INSERT INTO discord_linked_users (user_id, discord_id)
                    VALUES (?, ?)
                    ON DUPLICATE KEY UPDATE
                    discord_id = VALUES(discord_id)
                    "#,
                    [user_id.into(), discord_user_id.into()],
                    txn,
                )
                .await?;

                Ok::<(), InfraError>(())
            })
        })
        .await
        .map_err(Into::into)
    }

    async fn fetch_discord_user_id(
        &self,
        user: &User,
    ) -> Result<Option<DiscordUserId>, InfraError> {
        let user_id = user.id.to_string();

        Ok(self
            .read_only_transaction(|txn| {
                Box::pin(async move {
                    let query = query_one_and_values(
                        "SELECT discord_id FROM discord_linked_users WHERE user_id = ?",
                        [user_id.into()],
                        txn,
                    )
                    .await?;

                    query
                        .map(|rs| {
                            Ok::<DiscordUserId, InfraError>(DiscordUserId::new(
                                rs.try_get::<String>("", "discord_id")?,
                            ))
                        })
                        .transpose()
                })
            })
            .await?)
    }
}
