use dotenvy::dotenv;
use sea_orm::{Database, DatabaseConnection};
use std::env;

pub async fn database_connection() -> DatabaseConnection {
    dotenv().ok();

    let username = env::var("MYSQL_USER").expect("データベースのユーザー名を指定してください。");
    let password =
        env::var("MYSQL_PASSWORD").expect("データベースのパスワードを指定してください。");
    let host = env::var("MYSQL_HOST").expect("データベースのホストを指定してください。");
    let port = env::var("MYSQL_PORT").expect("データベースの接続ポートを指定してください。");
    let database_name = env::var("MYSQL_DATABASE").expect("データベース名を指定してください。");

    let database_url = format!("mysql://{username}:{password}@{host}:{port}/{database_name}");

    Database::connect(&database_url)
        .await
        .unwrap_or_else(|_| panic!("{database_url} に接続できませんでした。"))
}
