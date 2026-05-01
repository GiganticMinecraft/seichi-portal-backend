pub mod components;
pub mod config;
pub mod connection;
pub mod forms;
mod meilisearch_schemas;
pub mod notification;
pub mod search;
pub mod user;

use errors::infra::InfraError;

pub(crate) fn count_as_u32(count: i64, table: &str) -> Result<u32, InfraError> {
    u32::try_from(count).map_err(|_| InfraError::Unexpected {
        cause: format!("count overflow for {table}: {count}"),
    })
}
