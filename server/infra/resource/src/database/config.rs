use std::sync::LazyLock;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct MySQL {
    pub database: String,
    pub user: String,
    pub password: String,
    pub host: String,
    pub port: String,
}

pub static MYSQL: LazyLock<MySQL> =
    LazyLock::new(|| envy::prefixed("MYSQL_").from_env::<MySQL>().unwrap());

#[derive(Deserialize, Debug)]
pub struct Redis {
    pub host: String,
    pub port: String,
}

pub static REDIS: LazyLock<Redis> =
    LazyLock::new(|| envy::prefixed("REDIS_").from_env::<Redis>().unwrap());

#[derive(Deserialize, Debug)]
pub struct MeiliSearch {
    pub host: String,
    pub api_key: Option<String>,
}

pub static MEILISEARCH: LazyLock<MeiliSearch> = LazyLock::new(|| {
    envy::prefixed("MEILISEARCH_")
        .from_env::<MeiliSearch>()
        .unwrap()
});
