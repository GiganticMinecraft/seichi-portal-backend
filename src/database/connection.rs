use diesel::{Connection, MysqlConnection};
use dotenvy::dotenv;
use std::env;

pub fn database_connection() -> MysqlConnection {
    dotenv().ok();

    let database_url =
        env::var("DATABASE_URL").expect("データべースのURLを.envファイルに設定してください。");
    MysqlConnection::establish(&database_url)
        .expect(&format!("{} に接続できませんでした。", database_url))
}