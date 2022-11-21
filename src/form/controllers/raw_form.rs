use crate::form::domain::{Form, FormId, FormTitle};
use derive_getters::Getters;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Getters)]
pub struct RawForm {
    pub form_titles: Vec<String>,
    pub form_id: u8,
}

impl From<RawForm> for Form {
    fn from(f: RawForm) -> Self {
        Form::builder()
            .form_titles(
                f.form_titles()
                    .iter()
                    .map(|t| FormTitle::builder().title(t.to_string()))
                    .collect(),
            )
            .form_id(FormId::builder().form_id(*f.form_id()).build())
            .build()
    }
}
