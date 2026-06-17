use std::ops::Deref;

use errors::validation::{ValidationError, ValidationError::DuplicateElement};
#[cfg(any(test, feature = "arbitrary"))]
use proptest::{
    arbitrary::Arbitrary,
    collection::SizeRange,
    strategy::{BoxedStrategy, Strategy},
};
use serde::{Deserialize, Serialize};

/// 要素の順序を保持し、重複する要素を拒否する Vec です。
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct OrderedUniqueVec<T>(Vec<T>);

impl<T: PartialEq> OrderedUniqueVec<T> {
    pub fn try_new(value: Vec<T>) -> Result<Self, ValidationError> {
        if value
            .iter()
            .enumerate()
            .any(|(index, item)| value[..index].contains(item))
        {
            return Err(DuplicateElement);
        }

        Ok(Self(value))
    }

    pub fn empty() -> Self {
        Self(Vec::new())
    }
}

impl<T> OrderedUniqueVec<T> {
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }

    pub fn into_inner(self) -> Vec<T> {
        self.0
    }
}

impl<T> Deref for OrderedUniqueVec<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Serialize> Serialize for OrderedUniqueVec<T> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de, T> Deserialize<'de> for OrderedUniqueVec<T>
where
    T: Deserialize<'de> + PartialEq,
{
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<T>::deserialize(deserializer)
            .and_then(|value| OrderedUniqueVec::try_new(value).map_err(serde::de::Error::custom))
    }
}

#[cfg(any(test, feature = "arbitrary"))]
impl<T> Arbitrary for OrderedUniqueVec<T>
where
    T: Arbitrary + PartialEq + 'static,
    T::Strategy: 'static,
{
    type Parameters = (SizeRange, T::Parameters);
    type Strategy = BoxedStrategy<Self>;

    fn arbitrary_with(args: Self::Parameters) -> Self::Strategy {
        Vec::<T>::arbitrary_with(args)
            .prop_map(|value| {
                let mut unique = Vec::new();
                for item in value {
                    if !unique.contains(&item) {
                        unique.push(item);
                    }
                }
                OrderedUniqueVec(unique)
            })
            .boxed()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_empty_vec() {
        let value = OrderedUniqueVec::<i32>::try_new(vec![]).unwrap();

        assert!(value.as_slice().is_empty());
    }

    #[test]
    fn preserves_unique_values_in_order() {
        let value = OrderedUniqueVec::try_new(vec![3, 1, 2]).unwrap();

        assert_eq!(value.as_slice(), &[3, 1, 2]);
    }

    #[test]
    fn rejects_duplicate_values() {
        let result = OrderedUniqueVec::try_new(vec![1, 2, 1]);

        assert_eq!(result, Err(DuplicateElement));
    }

    #[test]
    fn serialize_deserialize() {
        let value = OrderedUniqueVec::try_new(vec![1, 2, 3]).unwrap();
        let serialized = serde_json::to_string(&value).unwrap();
        let deserialized: OrderedUniqueVec<i32> = serde_json::from_str(&serialized).unwrap();

        assert_eq!(serialized, "[1,2,3]");
        assert_eq!(value, deserialized);
    }

    #[test]
    fn deserialize_rejects_duplicate_values() {
        let result = serde_json::from_str::<OrderedUniqueVec<i32>>("[1,2,1]");

        assert!(result.is_err());
    }
}
