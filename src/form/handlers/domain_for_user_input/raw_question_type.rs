use serde::{Deserialize, Serialize};
use strum_macros::Display;

#[derive(Serialize, Deserialize, Display)]
pub enum RawQuestionType {
    #[strum(serialize = "text")]
    TEXT,
    #[strum(serialize = "pulldown")]
    PULLDOWN,
    #[strum(serialize = "checkbox")]
    CHECKBOX,
}
