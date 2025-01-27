use chrono::{DateTime, Utc};
#[cfg(test)]
use common::test_utils::arbitrary_opt_date_time;
use derive_getters::Getters;
use deriving_via::DerivingVia;
use errors::domain::DomainError;
#[cfg(test)]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};
use types::non_empty_string::NonEmptyString;

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Clone, DerivingVia, Default, Debug, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize(via: Option::<NonEmptyString>), Deserialize(via: Option::<NonEmptyString>
))]
pub struct DefaultAnswerTitle(Option<NonEmptyString>);

impl DefaultAnswerTitle {
    pub fn new(title: Option<NonEmptyString>) -> Self {
        Self(title)
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(
    Serialize, Deserialize, Debug, EnumString, Display, Copy, Clone, Default, PartialOrd, PartialEq,
)]
pub enum AnswerVisibility {
    PUBLIC,
    #[default]
    PRIVATE,
}

impl TryFrom<String> for AnswerVisibility {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        use std::str::FromStr;
        Self::from_str(&value).map_err(Into::into)
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Getters, Clone, Default, Debug, PartialEq)]
pub struct ResponsePeriod {
    #[cfg_attr(test, proptest(strategy = "arbitrary_opt_date_time()"))]
    #[serde(default)]
    start_at: Option<DateTime<Utc>>,
    #[cfg_attr(test, proptest(strategy = "arbitrary_opt_date_time()"))]
    #[serde(default)]
    end_at: Option<DateTime<Utc>>,
}

impl ResponsePeriod {
    pub fn try_new(
        start_at: Option<DateTime<Utc>>,
        end_at: Option<DateTime<Utc>>,
    ) -> Result<Self, DomainError> {
        match (start_at, end_at) {
            (Some(start_at), Some(end_at)) if start_at > end_at => {
                Err(DomainError::InvalidResponsePeriod)
            }
            _ => Ok(Self { start_at, end_at }),
        }
    }

    pub fn is_within_period(&self, now: DateTime<Utc>) -> bool {
        if let Some(start_at) = self.start_at {
            if start_at > now {
                return false;
            }
        }
        if let Some(end_at) = self.end_at {
            if end_at < now {
                return false;
            }
        }
        true
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Getters, Default, Debug, PartialEq)]
pub struct AnswerSettings {
    default_answer_title: DefaultAnswerTitle,
    visibility: AnswerVisibility,
    response_period: ResponsePeriod,
}

impl AnswerSettings {
    pub fn new(
        default_answer_title: DefaultAnswerTitle,
        visibility: AnswerVisibility,
        response_period: ResponsePeriod,
    ) -> Self {
        Self {
            default_answer_title,
            visibility,
            response_period,
        }
    }
}
