#[cfg(test)]
use common::test_utils::arbitrary_with_size;
use derive_getters::Getters;
use deriving_via::DerivingVia;
#[cfg(test)]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize, Serializer};
use strum_macros::EnumString;
use typed_builder::TypedBuilder;

#[cfg_attr(test, derive(Arbitrary))]
#[derive(DerivingVia, Clone, Copy, Debug, PartialOrd, PartialEq)]
#[deriving(From, Into)]
pub struct FormId(i32);

impl Serialize for FormId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(DerivingVia, TypedBuilder, Deserialize, Clone, Getters, Debug, PartialOrd, PartialEq)]
#[deriving(From, Into)]
pub struct FormName {
    #[builder(setter(into))]
    name: String,
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(TypedBuilder, Getters, Debug, PartialEq)]
pub struct Form {
    id: FormId,
    name: FormName,
    #[cfg_attr(test, proptest(strategy = "arbitrary_with_size(1..100)"))]
    questions: Vec<Question>,
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(TypedBuilder, Getters, Debug, PartialEq)]
pub struct Question {
    title: String,
    description: String,
    question_type: QuestionType,
    #[cfg_attr(test, proptest(strategy = "arbitrary_with_size(1..100)"))]
    choices: Vec<String>,
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Debug, EnumString, PartialOrd, PartialEq)]
#[strum(ascii_case_insensitive)]
pub enum QuestionType {
    TEXT,
    PULLDOWN,
    CHECKBOX,
}

impl TryFrom<String> for QuestionType {
    type Error = errors::domain::DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        use std::str::FromStr;
        Self::from_str(&value).map_err(Into::into)
    }
}

#[cfg(test)]
mod test {
    use proptest::{prop_assert_eq, proptest};
    use test_case::test_case;

    use super::*;

    #[test_case("TEXT"     => Ok(QuestionType::TEXT); "upper: TEXT")]
    #[test_case("text"     => Ok(QuestionType::TEXT); "lower: text")]
    #[test_case("PULLDOWN" => Ok(QuestionType::PULLDOWN); "upper: PULLDOWN")]
    #[test_case("pulldown" => Ok(QuestionType::PULLDOWN); "lower: pulldown")]
    #[test_case("CHECKBOX" => Ok(QuestionType::CHECKBOX); "upper: CHECKBOX")]
    #[test_case("checkbox" => Ok(QuestionType::CHECKBOX); "lower: checkbox")]
    fn string_to_question_type(input: &str) -> Result<QuestionType, errors::domain::DomainError> {
        input.to_owned().try_into()
    }

    proptest! {
        #[test]
        fn string_into_from_name(name: String) {
            let form_name: FormName = name.to_owned().into();
            prop_assert_eq!(form_name, FormName::builder().name(name).build());
        }
    }
}
