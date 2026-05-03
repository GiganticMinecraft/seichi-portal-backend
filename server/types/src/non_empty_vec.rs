use std::ops::Deref;

use errors::validation::{ValidationError, ValidationError::EmptyValue};
#[cfg(any(test, feature = "arbitrary"))]
use proptest::{
    arbitrary::Arbitrary,
    collection::SizeRange,
    strategy::{BoxedStrategy, Strategy},
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct NonEmptyVec<T>(Vec<T>);

impl<T> NonEmptyVec<T> {
    pub fn try_new(value: Vec<T>) -> Result<Self, ValidationError> {
        if value.is_empty() {
            return Err(EmptyValue);
        }

        Ok(Self(value))
    }

    pub fn into_inner(self) -> Vec<T> {
        self.0
    }
}

impl<T> Deref for NonEmptyVec<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Serialize> Serialize for NonEmptyVec<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for NonEmptyVec<T> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<T>::deserialize(deserializer)
            .and_then(|value| NonEmptyVec::try_new(value).map_err(serde::de::Error::custom))
    }
}

#[cfg(any(test, feature = "arbitrary"))]
impl<T> Arbitrary for NonEmptyVec<T>
where
    T: Arbitrary + 'static,
    T::Strategy: 'static,
{
    type Parameters = (SizeRange, T::Parameters);
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(args: Self::Parameters) -> Self::Strategy {
        Vec::<T>::arbitrary_with(args)
            .prop_filter_map("non-empty vec", |value| NonEmptyVec::try_new(value).ok())
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_empty_vec_rejects_empty_vec() {
        assert_eq!(NonEmptyVec::<i32>::try_new(vec![]), Err(EmptyValue));
    }

    #[test]
    fn serialize_deserialize() {
        let value = NonEmptyVec::try_new(vec![1, 2, 3]).unwrap();
        let serialized = serde_json::to_string(&value).unwrap();
        let deserialized: NonEmptyVec<i32> = serde_json::from_str(&serialized).unwrap();

        assert_eq!(value, deserialized);
    }
}
