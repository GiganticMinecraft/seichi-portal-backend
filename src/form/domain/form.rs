use crate::form::domain::{FormId, FormTitle};
use derive_getters::Getters;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Getters)]
pub struct Form {
    pub form_titles: Vec<FormTitle>,
    pub form_id: FormId,
}
