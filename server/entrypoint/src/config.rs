use once_cell::sync::Lazy;

#[derive(serde::Deserialize, Debug)]
pub struct Http {
    pub port: String,
}

pub static HTTP: Lazy<Http> = Lazy::new(|| envy::prefixed("HTTP_").from_env::<Http>().unwrap());

#[derive(serde::Deserialize, Debug)]
pub struct Env {
    pub name: String,
}

pub static ENV: Lazy<Env> = Lazy::new(|| envy::prefixed("ENV_").from_env::<Env>().unwrap());
