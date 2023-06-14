use chrono::Utc;
use proptest::prelude::Just;
use proptest::{arbitrary::Arbitrary, collection::SizeRange, strategy::Strategy};

pub fn arbitrary_with_size<A: Arbitrary>(
    size_range: impl Into<SizeRange>,
) -> impl Strategy<Value = Vec<A>> {
    proptest::collection::vec(A::arbitrary(), size_range)
}

pub fn arbitrary_date_time() -> impl Strategy<Value = chrono::DateTime<Utc>> {
    Just(Utc::now())
}
