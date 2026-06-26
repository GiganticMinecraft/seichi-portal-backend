pub mod natural_f32;
pub mod non_empty_string;
pub mod non_empty_vec;
pub mod ordered_unique_vec;

#[cfg(feature = "arbitrary")]
use common::test_utils::arbitrary_uuid_v7;
use deriving_via::DerivingVia;
#[cfg(feature = "arbitrary")]
use proptest_derive::Arbitrary;
use utoipa::PartialSchema;
use uuid::Uuid;

impl<T: 'static> utoipa::ToSchema for IntegerId<T> {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("IntegerId")
    }
}

impl<T: 'static> PartialSchema for IntegerId<T> {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        utoipa::openapi::ObjectBuilder::new()
            .schema_type(utoipa::openapi::schema::SchemaType::new(
                utoipa::openapi::schema::Type::Integer,
            ))
            .format(Some(utoipa::openapi::SchemaFormat::KnownFormat(
                utoipa::openapi::KnownFormat::Int32,
            )))
            .into()
    }
}

impl<T: 'static> utoipa::ToSchema for Id<T> {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("Id")
    }
}

impl<T: 'static> PartialSchema for Id<T> {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        utoipa::openapi::ObjectBuilder::new()
            .schema_type(utoipa::openapi::schema::SchemaType::new(
                utoipa::openapi::schema::Type::String,
            ))
            .format(Some(utoipa::openapi::SchemaFormat::KnownFormat(
                utoipa::openapi::KnownFormat::Uuid,
            )))
            .into()
    }
}

#[derive(DerivingVia, Debug, PartialOrd, PartialEq)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
#[deriving(
    From,
    Into,
    Copy,
    Default,
    IntoInner(via: i32),
    Display(via: i32),
    Serialize(via: i32),
    Deserialize(via: i32)
)]
pub struct IntegerId<T>(#[underlying] i32, std::marker::PhantomData<T>);

#[derive(DerivingVia, Debug)]
#[cfg_attr(feature = "arbitrary", derive(Arbitrary))]
#[deriving(From, Into, Copy, Default, IntoInner(via: Uuid), Display(via: Uuid), Serialize(via: Uuid
), Deserialize(via: Uuid))]
pub struct Id<T>(
    #[cfg_attr(feature = "arbitrary", proptest(strategy = "arbitrary_uuid_v7()"))]
    #[underlying]
    Uuid,
    std::marker::PhantomData<T>,
);

impl<T> Id<T> {
    pub fn new() -> Self {
        Self(Uuid::now_v7(), std::marker::PhantomData)
    }
}

impl<T> Eq for Id<T> {}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T> PartialOrd for Id<T> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Id<T> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.cmp(&other.0)
    }
}

impl<T> std::hash::Hash for Id<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(&self.0, state);
    }
}
