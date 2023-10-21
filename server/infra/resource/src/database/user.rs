use async_trait::async_trait;
use domain::user::models::User;
use errors::infra::InfraError;
use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

use crate::database::{components::UserDatabase, connection::ConnectionPool};

#[async_trait]
impl UserDatabase for ConnectionPool {
    async fn upsert_user(&self, user: &User) -> Result<(), InfraError> {
        self.pool
            .execute(Statement::from_sql_and_values(
                DatabaseBackend::MySql,
                "INSERT INTO users (uuid, name) VALUES (UUID_TO_BIN(?), ?)
                        ON DUPLICATE KEY UPDATE
                        name = VALUES(name)",
                [user.id.to_string().into(), user.name.to_owned().into()],
            ))
            .await?;

        Ok(())
    }
}
