use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Deserialize, Debug)]
pub struct RabbitMQ {
    pub user: String,
    pub password: String,
    pub host: String,
    pub port: String,
    pub routing_key: String,
}

pub static RABBITMQ: LazyLock<RabbitMQ> =
    LazyLock::new(|| envy::prefixed("RABBITMQ_").from_env::<RabbitMQ>().unwrap());
