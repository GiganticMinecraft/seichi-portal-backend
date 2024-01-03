use std::str::FromStr;

use async_trait::async_trait;
use domain::user::models::{Role, User};
use errors::infra::InfraError;
use uuid::Uuid;

use crate::database::{components::UserDatabase, connection::ConnectionPool};

#[async_trait]
impl UserDatabase for ConnectionPool {
    async fn find_by(&self, uuid: Uuid) -> Result<Option<User>, InfraError> {
        let query = self
            .query_one_and_values(
                "SELECT name, role FROM users WHERE uuid = ?",
                [uuid.to_string().into()],
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

        Ok(user)
    }

    async fn upsert_user(&self, user: &User) -> Result<(), InfraError> {
        self.execute_and_values(
            "INSERT INTO users (uuid, name, role) VALUES (?, ?, ?)
                        ON DUPLICATE KEY UPDATE
                        name = VALUES(name)",
            [
                user.id.to_string().into(),
                user.name.to_owned().into(),
                user.role.to_string().into(),
            ],
        )
        .await?;

        Ok(())
    }

    async fn patch_user_role(&self, uuid: Uuid, role: Role) -> Result<(), InfraError> {
        self.execute_and_values(
            "UPDATE users SET role = ? WHERE uuid = ?",
            [role.to_string().into(), uuid.to_string().into()],
        )
        .await?;

        Ok(())
    }
}
