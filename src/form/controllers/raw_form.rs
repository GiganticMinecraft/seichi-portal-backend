use crate::form::domain::Form;
use derive_getters::Getters;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Getters)]
pub struct RawForm {
    pub form_titles: Vec<String>,
    pub form_id: u8,
}

impl From<RawForm> for Form {
    fn from(f: RawForm) -> Self {
        Form {
            form_titles: f
                .form_titles()
                .iter()
                .map(|t| FormTitle {
                    title: t.to_string(),
                })
                .collect(),
            form_id: FormId {
                form_id: *f.form_id(),
            },
        }
    }
}
