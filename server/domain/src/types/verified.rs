use async_trait::async_trait;
use errors::Error;

#[derive(Debug)]
pub struct Verified<T> {
    inner: T,
}

impl<T> Verified<T> {
    pub fn into_inner(self) -> T {
        self.inner
    }

    pub fn inner(&self) -> &T {
        &self.inner
    }
}

#[async_trait]
pub trait Verifier<T> {
    async fn verify(self) -> Result<Verified<T>, Error>;

    fn new(inner: T) -> Verified<T> {
        Verified { inner }
    }
}
