use async_trait::async_trait;
use deriving_via::DerivingVia;
#[cfg(feature = "arbitrary")]
use proptest_derive::Arbitrary;

#[derive(DerivingVia, Debug, PartialOrd, PartialEq)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
#[deriving(From, Into, Copy, Default, IntoInner(via: i32), Display(via: i32), Serialize(via: i32), Deserialize(via: i32))]
pub struct Id<T>(#[underlying] i32, std::marker::PhantomData<T>);

#[async_trait]
pub trait Resolver<T, Error, Repo> {
    async fn resolve(&self, repo: &Repo) -> Result<Option<T>, Error>;
}
