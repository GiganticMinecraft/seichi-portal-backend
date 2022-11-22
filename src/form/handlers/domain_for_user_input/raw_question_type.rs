use serde::{Deserialize, Serialize};
use strum_macros::Display;

#[derive(Serialize, Deserialize, Display)]
pub enum QuestionType {
    #[strum(serialize = "text")]
    TEXT,
    #[strum(serialize = "pulldown")]
    PULLDOWN,
    #[strum(serialize = "checkbox")]
    CHECKBOX,
}
