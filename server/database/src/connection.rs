use sea_orm::{Database, DatabaseConnection};
use crate::config::{MYSQL, MySQL};

pub async fn database_connection() -> DatabaseConnection {
    let MySQL { user, password, host, port, database, .. } = &*MYSQL;

    let database_url = format!("mysql://{user}:{password}@{host}:{port}/{database}");

    Database::connect(&database_url)
        .await
        .unwrap_or_else(|_| panic!("Cannot establish connect to {database_url}."))
}
