use once_cell::sync::Lazy;

#[derive(serde::Deserialize, Debug)]
pub struct Http {
    pub port: String,
}

pub static HTTP: Lazy<Http> = Lazy::new(|| envy::prefixed("HTTP_").from_env::<Http>().unwrap());

#[derive(serde::Deserialize, Debug)]
pub struct Environment {
    pub name: String,
}

pub static ENVIRONMENT: Lazy<Environment> =
    Lazy::new(|| envy::prefixed("ENV_").from_env::<Environment>().unwrap());
