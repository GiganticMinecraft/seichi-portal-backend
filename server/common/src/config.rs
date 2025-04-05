use std::sync::LazyLock;

use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Http {
    pub port: String,
}

pub static HTTP: LazyLock<Http> =
    LazyLock::new(|| envy::prefixed("HTTP_").from_env::<Http>().unwrap());

#[derive(Deserialize, Debug)]
pub struct Env {
    pub name: String,
}

pub static ENV: LazyLock<Env> = LazyLock::new(|| envy::prefixed("ENV_").from_env::<Env>().unwrap());

#[derive(Deserialize, Debug)]
pub struct Frontend {
    pub url: String,
}

pub static FRONTEND: LazyLock<Frontend> =
    LazyLock::new(|| envy::prefixed("FRONTEND_").from_env::<Frontend>().unwrap());
