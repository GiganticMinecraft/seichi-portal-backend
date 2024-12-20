use errors::validation::ValidationError;
use errors::validation::ValidationError::EmptyValue;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::str::FromStr;

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct NonEmptyString(String);

impl NonEmptyString {
    pub fn try_new(value: String) -> Result<Self, ValidationError> {
        if value.is_empty() {
            return Err(EmptyValue);
        }

        Ok(Self(value))
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl FromStr for NonEmptyString {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        NonEmptyString::try_new(s.to_string())
    }
}

impl TryInto<NonEmptyString> for String {
    type Error = ValidationError;

    fn try_into(self) -> Result<NonEmptyString, Self::Error> {
        NonEmptyString::try_new(self)
    }
}

impl Deref for NonEmptyString {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Serialize for NonEmptyString {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}

impl<'de> Deserialize<'de> for NonEmptyString {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)
            .and_then(|value| NonEmptyString::try_new(value).map_err(serde::de::Error::custom))
    }
}

#[cfg(feature = "proptest")]
#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn non_empty_string(value in "\\PC*") {
            let result = NonEmptyString::try_new(value.clone());

            if value.is_empty() {
                prop_assert_eq!(result, Err(EmptyValue));
            } else {
                prop_assert_eq!(result, Ok(NonEmptyString(value)));
            }
        }
    }

    #[test]
    fn serialize_deserialize() {
        let value = NonEmptyString::try_new("value".to_string()).unwrap();
        let serialized = serde_json::to_string(&value).unwrap();
        let deserialized: NonEmptyString = serde_json::from_str(&serialized).unwrap();

        assert_eq!(value, deserialized);
    }
}
