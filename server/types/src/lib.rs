use async_trait::async_trait;
use deriving_via::DerivingVia;
#[cfg(feature = "arbitrary")]
use proptest_derive::Arbitrary;
use uuid::Uuid;

#[derive(DerivingVia, Debug, PartialOrd, PartialEq)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
#[deriving(From, Into, Copy, Default, IntoInner(via: i32), Display(via: i32), Serialize(via: i32), Deserialize(via: i32))]
pub struct IntegerId<T>(#[underlying] i32, std::marker::PhantomData<T>);

#[derive(DerivingVia, Debug, PartialOrd, PartialEq)]
#[deriving(From, Into, Copy, Default, IntoInner(via: Uuid), Display(via: Uuid), Serialize(via: Uuid), Deserialize(via: Uuid))]
pub struct Id<T>(#[underlying] Uuid, std::marker::PhantomData<T>);

impl<T> Id<T> {
    pub fn new() -> Self {
        Self(Uuid::now_v7(), std::marker::PhantomData)
    }
}

#[async_trait]
pub trait Resolver<T, Error, Repo> {
    async fn resolve(&self, repo: &Repo) -> Result<Option<T>, Error>;
}
