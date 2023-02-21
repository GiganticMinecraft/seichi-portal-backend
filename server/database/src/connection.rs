use dotenvy::dotenv;
use sea_orm::{Database, DatabaseConnection};
use std::env;

pub async fn database_connection() -> DatabaseConnection {
    dotenv().ok();

    let database_url =
        env::var("DATABASE_URL").expect("データべースのURLを.envファイルに設定してください。");
    Database::connect(&database_url)
        .await
        .unwrap_or_else(|_| panic!("{database_url} に接続できませんでした。"))
}
