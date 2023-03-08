use proptest::{arbitrary::Arbitrary, collection::SizeRange, strategy::Strategy};

pub fn arbitrary_with_size<A: Arbitrary>(
    size_range: impl Into<SizeRange>,
) -> impl Strategy<Value = Vec<A>> {
    proptest::collection::vec(A::arbitrary(), size_range)
}