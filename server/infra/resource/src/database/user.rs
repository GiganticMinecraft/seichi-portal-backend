use std::str::FromStr;

use async_trait::async_trait;
use chrono::Utc;
use domain::user::models::{ActiveUser, DiscordUser, Role};
use errors::infra::InfraError;
use itertools::Itertools;
use redis::Commands;
use sha256::digest;
use sqlx::{Row, query};
use uuid::Uuid;

use crate::{
    database::{
        components::UserDatabase,
        connection::{ConnectionPool, redis_connection},
        count::count_as_u32,
    },
    records::DiscordUserRecord,
};

#[async_trait]
impl UserDatabase for ConnectionPool {
    async fn find_by(&self, uuid: Uuid) -> Result<Option<ActiveUser>, InfraError> {
        Ok(self
            .read_only_transaction(|txn| {
                Box::pin(async move {
                    let query = sqlx::query!(
                        "SELECT name, role FROM users WHERE id = ?",
                        uuid.to_string()
                    )
                    .fetch_optional(&mut **txn)
                    .await?;

                    let user = query
                        .map(|row| {
                            Ok::<ActiveUser, InfraError>(ActiveUser::new(
                                row.name,
                                uuid.into(),
                                Role::from_str(&row.role)?,
                            ))
                        })
                        .transpose()?;

                    Ok::<_, InfraError>(user)
                })
            })
            .await?)
    }

    async fn find_by_ids(&self, uuids: Vec<Uuid>) -> Result<Vec<ActiveUser>, InfraError> {
        if uuids.is_empty() {
            return Ok(Vec::new());
        }

        let uuid_strings = uuids.into_iter().map(|id| id.to_string()).collect_vec();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let sql = format!(
                    "SELECT id, name, role FROM users WHERE id IN ({})",
                    std::iter::repeat_n("?", uuid_strings.len()).join(", ")
                );

                let rows = uuid_strings
                    .iter()
                    .fold(query(&sql), |query, uuid| query.bind(uuid))
                    .fetch_all(&mut **txn)
                    .await?;

                rows.into_iter()
                    .map(|row| {
                        let id: String = row.try_get("id")?;
                        let name: String = row.try_get("name")?;
                        let role: String = row.try_get("role")?;
                        Ok::<ActiveUser, InfraError>(ActiveUser::new(
                            name,
                            Uuid::parse_str(&id)?.into(),
                            Role::from_str(&role)?,
                        ))
                    })
                    .collect::<Result<Vec<ActiveUser>, _>>()
            })
        })
        .await
    }

    async fn upsert_user(&self, user: &ActiveUser) -> Result<(), InfraError> {
        let user_id = user.id().to_string();
        let user_name = user.name().to_owned();
        let user_role = user.role().to_string();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "INSERT INTO users (id, name, role) VALUES (?, ?, ?)
                        ON DUPLICATE KEY UPDATE
                        name = VALUES(name)",
                    user_id,
                    user_name,
                    user_role,
                )
                .execute(&mut **txn)
                .await?;

                Ok::<(), InfraError>(())
            })
        })
        .await
    }

    async fn patch_user_role(&self, uuid: Uuid, role: Role) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "UPDATE users SET role = ? WHERE id = ?",
                    role.to_string(),
                    uuid.to_string(),
                )
                .execute(&mut **txn)
                .await?;

                Ok::<(), InfraError>(())
            })
        })
        .await
    }

    async fn fetch_all_users(&self) -> Result<Vec<ActiveUser>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let query = sqlx::query!("SELECT id, name, role FROM users")
                    .fetch_all(&mut **txn)
                    .await?;

                let users = query
                    .into_iter()
                    .map(|row| {
                        Ok::<ActiveUser, InfraError>(ActiveUser::new(
                            row.name,
                            Uuid::parse_str(&row.id)?.into(),
                            Role::from_str(&row.role)?,
                        ))
                    })
                    .collect::<Result<Vec<ActiveUser>, InfraError>>()?;

                Ok::<_, InfraError>(users)
            })
        })
        .await
    }

    async fn start_user_session(
        &self,
        xbox_token: String,
        user: &ActiveUser,
        expires: u32,
    ) -> Result<String, InfraError> {
        let now = Utc::now().timestamp_millis();
        let session_id = digest(format!("{xbox_token}{now}"));

        let mut redis_connection = redis_connection().await;

        let user_json = serde_json::to_string(user)?;
        redis_connection.set_ex::<&str, String, ()>(&session_id, user_json, expires as u64)?;

        Ok(session_id)
    }

    async fn fetch_user_by_session_id(
        &self,
        session_id: String,
    ) -> Result<Option<ActiveUser>, InfraError> {
        let mut redis_connection = redis_connection().await;

        let result: Option<String> = redis_connection.get(&session_id)?;
        let user = result.and_then(|s| serde_json::from_str::<ActiveUser>(&s).ok());

        Ok(user)
    }

    async fn end_user_session(&self, session_id: String) -> Result<(), InfraError> {
        let mut redis_connection = redis_connection().await;

        redis_connection.del::<&str, ()>(&session_id)?;

        Ok(())
    }

    async fn link_discord_user(
        &self,
        discord_user: &DiscordUser,
        user: &ActiveUser,
    ) -> Result<(), InfraError> {
        let user_id = user.id().to_string();
        let discord_user_id = discord_user.id().to_owned().into_inner();
        let discord_username = discord_user.name().to_owned().into_inner().to_owned();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    r#"INSERT INTO discord_linked_users (user_id, discord_id, discord_username)
                    VALUES (?, ?, ?)
                    ON DUPLICATE KEY UPDATE
                    discord_id = VALUES(discord_id),
                    discord_username = VALUES(discord_username)
                    "#,
                    user_id,
                    discord_user_id,
                    discord_username,
                )
                .execute(&mut **txn)
                .await?;

                Ok::<(), InfraError>(())
            })
        })
        .await
    }

    async fn unlink_discord_user(&self, user: &ActiveUser) -> Result<(), InfraError> {
        let user_id = user.id().to_string();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "DELETE FROM discord_linked_users WHERE user_id = ?",
                    user_id,
                )
                .execute(&mut **txn)
                .await?;

                Ok::<(), InfraError>(())
            })
        })
        .await
    }

    async fn fetch_discord_user(
        &self,
        user: &ActiveUser,
    ) -> Result<Option<DiscordUserRecord>, InfraError> {
        let user_id = user.id().to_string();

        Ok(self
            .read_only_transaction(|txn| {
                Box::pin(async move {
                    let query = sqlx::query!(
                        "SELECT discord_id, discord_username FROM discord_linked_users WHERE \
                         user_id = ?",
                        user_id,
                    )
                    .fetch_optional(&mut **txn)
                    .await?;

                    query
                        .map(|row| {
                            Ok::<_, InfraError>(DiscordUserRecord {
                                user_id: row.discord_id,
                                username: row.discord_username,
                            })
                        })
                        .transpose()
                })
            })
            .await?)
    }

    async fn fetch_size(&self) -> Result<u32, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let size = sqlx::query_scalar!("SELECT COUNT(*) AS `count!: i64` FROM users")
                    .fetch_one(&mut **txn)
                    .await?;

                count_as_u32(size, "users")
            })
        })
        .await
    }
}
