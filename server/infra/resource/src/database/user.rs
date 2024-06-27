use std::str::FromStr;

use async_trait::async_trait;
use domain::user::models::{Role, User};
use errors::infra::InfraError;
use uuid::Uuid;

use crate::database::{
    components::UserDatabase,
    connection::{execute_and_values, query_one_and_values, ConnectionPool},
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
                                uuid,
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
            user.uuid.to_string().into(),
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
}
