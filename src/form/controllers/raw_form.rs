use derive_getters::Getters;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Getters)]
pub struct RawForm {
    pub form_titles: Vec<String>,
    pub form_id: u8,
}
