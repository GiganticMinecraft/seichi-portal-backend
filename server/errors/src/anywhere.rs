use anyhow;
use tracing;

use thiserror::{Error};

#[derive(Error, Debug)]
#[error(transparent)]
struct Source(#[from] anyhow::Error);

#[derive(Debug)]
pub struct Error {
    source: Source,
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.source()
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(fmt, "cause:\n\t{}", self.source)
    }
}

impl<E: std::fmt::Debug> From<E> for Error
    where
        Source: From<E>,
{
    fn from(source: E) -> Self {
        let source: Source = source.into();
        tracing::error!("{}", &source);
        Self { source }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
