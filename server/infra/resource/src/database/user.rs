use async_trait::async_trait;
use domain::user::models::User;
use errors::infra::InfraError;

use crate::database::{components::UserDatabase, connection::ConnectionPool};

#[async_trait]
impl UserDatabase for ConnectionPool {
    async fn upsert_user(&self, user: &User) -> Result<(), InfraError> {
        self.execute_and_values(
            "INSERT INTO users (uuid, name, role) VALUES (?, ?, ?)
                        ON DUPLICATE KEY UPDATE
                        name = VALUES(name),
                        role = VALUES(role)",
            [
                user.id.to_string().into(),
                user.name.to_owned().into(),
                user.role.to_string().into(),
            ],
        )
        .await?;

        Ok(())
    }
}
