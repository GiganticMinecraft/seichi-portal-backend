use dotenvy::dotenv;
use once_cell::sync::Lazy;

#[derive(serde::Deserialize, Debug)]
pub struct Http {
    pub port: String,
}

pub static HTTP: Lazy<Http> = Lazy::new(|| {
    dotenv().expect("Cannot find `.env` file.");
    envy::prefixed("HTTP_").from_env::<Http>().unwrap()
});
