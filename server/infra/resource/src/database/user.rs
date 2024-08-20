use std::str::FromStr;

use async_trait::async_trait;
use chrono::Utc;
use domain::user::models::{Role, User};
use errors::infra::InfraError;
use redis::{Commands, JsonCommands};
use sha256::digest;
use uuid::Uuid;

use crate::database::{
    components::UserDatabase,
    connection::{execute_and_values, query_one_and_values, redis_connection, ConnectionPool},
};

#[async_trait]
impl UserDatabase for ConnectionPool {
    async fn find_by(&self, uuid: Uuid) -> Result<Option<User>, InfraError> {
        Ok(self
            .read_only_transaction(|txn| {
                Box::pin(async move {
                    let query = query_one_and_values(
                        "SELECT name, role FROM users WHERE uuid = ?",
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
                    "INSERT INTO users (uuid, name, role) VALUES (?, ?, ?)
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
                    "UPDATE users SET role = ? WHERE uuid = ?",
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

    async fn start_user_session(
        &self,
        xbox_token: String,
        user: &User,
    ) -> Result<String, InfraError> {
        let now = Utc::now().timestamp_millis();
        let session_id = digest(format!("{xbox_token}{now}"));

        let mut redis_connection = redis_connection().await;

        let _: () = redis_connection.json_set(&session_id, "$", user)?;

        let half_an_hour = 1800;
        let _: () = redis_connection.expire(&session_id, half_an_hour)?;
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

    async fn update_user_session(&self, session_id: String) -> Result<(), InfraError> {
        let mut redis_connection = redis_connection().await;

        let half_an_hour = 1800;
        let _: () = redis_connection.expire(&session_id, half_an_hour)?;

        Ok(())
    }

    async fn end_user_session(&self, session_id: String) -> Result<(), InfraError> {
        let mut redis_connection = redis_connection().await;

        let _: () = redis_connection.del(&session_id)?;

        Ok(())
    }
}
