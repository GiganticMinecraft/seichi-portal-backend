use chrono::{DateTime, Utc};
#[cfg(test)]
use common::test_utils::{arbitrary_date_time, arbitrary_with_size};
use derive_getters::Getters;
use deriving_via::DerivingVia;
#[cfg(test)]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};
use strum_macros::EnumString;
use typed_builder::TypedBuilder;

#[cfg_attr(test, derive(Arbitrary))]
#[derive(DerivingVia, Clone, Copy, Debug, PartialOrd, PartialEq)]
#[deriving(From, Into, Serialize(via: i32))]
pub struct FormId(pub i32);

#[derive(Deserialize)]
pub struct OffsetAndLimit {
    pub offset: i32,
    pub limit: i32,
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(
    DerivingVia, TypedBuilder, Serialize, Deserialize, Clone, Getters, Debug, PartialOrd, PartialEq,
)]
#[deriving(From, Into)]
pub struct FormTitle {
    #[builder(setter(into))]
    title: String,
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(TypedBuilder, Serialize, Getters, Debug, PartialEq)]
pub struct Form {
    id: FormId,
    title: FormTitle,
    #[cfg_attr(test, proptest(strategy = "arbitrary_with_size(1..100)"))]
    questions: Vec<Question>,
    metadata: FormMeta,
    settings: FormSettings,
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(TypedBuilder, Serialize, Getters, Debug, PartialEq)]
pub struct Question {
    title: String,
    description: Option<String>,
    question_type: QuestionType,
    #[cfg_attr(test, proptest(strategy = "arbitrary_with_size(1..100)"))]
    choices: Vec<String>,
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Debug, Serialize, EnumString, PartialOrd, PartialEq)]
#[strum(ascii_case_insensitive)]
pub enum QuestionType {
    TEXT,
    SINGLE,
    MULTIPLE,
}

impl TryFrom<String> for QuestionType {
    type Error = errors::domain::DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        use std::str::FromStr;
        Self::from_str(&value).map_err(Into::into)
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, TypedBuilder, Debug, PartialEq)]
pub struct FormMeta {
    #[cfg_attr(test, proptest(strategy = "arbitrary_date_time()"))]
    created_at: DateTime<Utc>,
    #[cfg_attr(test, proptest(strategy = "arbitrary_date_time()"))]
    update_at: DateTime<Utc>,
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Debug, PartialEq, TypedBuilder)]
pub struct FormSettings {
    response_period: Option<ResponsePeriod>,
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(TypedBuilder, Serialize, Debug, PartialEq)]
pub struct ResponsePeriod {
    #[cfg_attr(test, proptest(strategy = "arbitrary_date_time()"))]
    start_at: DateTime<Utc>,
    #[cfg_attr(test, proptest(strategy = "arbitrary_date_time()"))]
    end_at: DateTime<Utc>,
}

#[cfg(test)]
mod test {

    use proptest::{prop_assert_eq, proptest};
    use serde_json::json;
    use test_case::test_case;

    use super::*;

    #[test_case("TEXT"     => Ok(QuestionType::TEXT); "upper: TEXT")]
    #[test_case("text"     => Ok(QuestionType::TEXT); "lower: text")]
    #[test_case("SINGLE" => Ok(QuestionType::SINGLE); "upper: SINGLE")]
    #[test_case("single" => Ok(QuestionType::SINGLE); "lower: single")]
    #[test_case("MULTIPLE" => Ok(QuestionType::MULTIPLE); "upper: MULTIPLE")]
    #[test_case("multiple" => Ok(QuestionType::MULTIPLE); "lower: multiple")]
    fn string_to_question_type(input: &str) -> Result<QuestionType, errors::domain::DomainError> {
        input.to_owned().try_into()
    }

    proptest! {
        #[test]
        fn string_into_from_title(title: String) {
            let form_title: FormTitle = title.to_owned().into();
            prop_assert_eq!(form_title, FormTitle::builder().title(title).build());
        }
    }

    proptest! {
        #[test]
        fn serialize_from_id(id: i32) {
            let form_id: FormId = id.into();
            prop_assert_eq!(json!({"id":form_id}).to_string(), format!(r#"{{"id":{id}}}"#));
        }
    }
}
