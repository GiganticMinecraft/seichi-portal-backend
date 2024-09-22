use serde::{Deserialize, Serialize};

use crate::form::models::Answer;
use crate::{
    form::models::{Form, Label},
    user::models::User,
};

#[derive(Serialize, Debug, PartialEq)]
pub struct FullSearch {
    pub forms: Vec<Form>,
    pub users: Vec<User>,
    pub answers: Vec<Answer>,
    pub label_for_forms: Vec<Label>,
    pub label_for_answers: Vec<Label>,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct SearchQuery {
    #[serde(default)]
    pub query: Option<String>,
}
