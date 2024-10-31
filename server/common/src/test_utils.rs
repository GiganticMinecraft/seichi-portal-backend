use chrono::Utc;
use proptest::{arbitrary::Arbitrary, collection::SizeRange, prelude::Just, strategy::Strategy};
use uuid::Uuid;

pub fn arbitrary_with_size<A: Arbitrary>(
    size_range: impl Into<SizeRange>,
) -> impl Strategy<Value = Vec<A>> {
    proptest::collection::vec(A::arbitrary(), size_range)
}

pub fn arbitrary_date_time() -> impl Strategy<Value = chrono::DateTime<Utc>> {
    Just(Utc::now())
}

pub fn arbitrary_opt_date_time() -> impl Strategy<Value = Option<chrono::DateTime<Utc>>> {
    Just(Some(Utc::now()))
}

pub fn arbitrary_uuid_v4() -> impl Strategy<Value = Uuid> {
    Just(Uuid::new_v4())
}

pub fn arbitrary_uuid_v7() -> impl Strategy<Value = Uuid> {
    Just(Uuid::now_v7())
}
