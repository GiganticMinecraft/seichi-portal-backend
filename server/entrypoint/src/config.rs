use once_cell::sync::Lazy;

#[derive(serde::Deserialize, Debug)]
pub struct Http {
    pub port: String,
}

pub static HTTP: Lazy<Http> = Lazy::new(|| envy::prefixed("HTTP_").from_env::<Http>().unwrap());

#[derive(serde::Deserialize, Debug)]
pub struct ServerEnvironment {
    pub name: String,
}

pub static NAME: Lazy<ServerEnvironment> = Lazy::new(|| {
    envy::prefixed("SRV_ENV_")
        .from_env::<ServerEnvironment>()
        .unwrap()
});
