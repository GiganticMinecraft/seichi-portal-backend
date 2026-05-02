use anyhow::{Context, Result};
use sqlx::mysql::MySqlPoolOptions;
use std::env;

fn database_url() -> Result<String> {
    match env::var("DATABASE_URL") {
        Ok(url) => Ok(url),
        Err(_) => {
            let user = env::var("MYSQL_USER").context("MYSQL_USER is not set")?;
            let password = env::var("MYSQL_PASSWORD").context("MYSQL_PASSWORD is not set")?;
            let host = env::var("MYSQL_HOST").context("MYSQL_HOST is not set")?;
            let port = env::var("MYSQL_PORT").context("MYSQL_PORT is not set")?;
            let database = env::var("MYSQL_DATABASE").context("MYSQL_DATABASE is not set")?;
            Ok(format!(
                "mysql://{user}:{password}@{host}:{port}/{database}"
            ))
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let database_url = database_url()?;
    let pool = MySqlPoolOptions::new().connect(&database_url).await?;

    match env::args().nth(1).as_deref() {
        None | Some("up") | Some("run") => migration::MIGRATOR.run(&pool).await?,
        Some("down") | Some("revert") => migration::MIGRATOR.undo(&pool, 0).await?,
        Some(command) => anyhow::bail!("unsupported migration command: {command}"),
    }

    Ok(())
}
