use once_cell::sync::Lazy;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct MySQL {
    pub root_password: String,
    pub database: String,
    pub user: String,
    pub password: String,
    pub host: String,
    pub port: String,
}

pub static MYSQL: Lazy<MySQL> = Lazy::new(|| envy::prefixed("MYSQL_").from_env::<MySQL>().unwrap());
