use once_cell::sync::Lazy;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct MySQL {
    pub database: String,
    pub user: String,
    pub password: String,
    pub host: String,
    pub port: String,
}

pub static MYSQL: Lazy<MySQL> = Lazy::new(|| envy::prefixed("MYSQL_").from_env::<MySQL>().unwrap());

#[derive(Deserialize, Debug)]
pub struct Redis {
    pub host: String,
    pub port: String,
}

pub static REDIS: Lazy<Redis> = Lazy::new(|| envy::prefixed("REDIS_").from_env::<Redis>().unwrap());
