use crate::form::domain::{Form, FormId, FormName};
use derive_getters::Getters;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Getters)]
pub struct RawForm {
    form_name: String,
}

impl From<RawForm> for Form {
    fn from(f: RawForm) -> Self {
        Form::builder()
            .form_name(FormName::builder().name(f.form_name().to_string()).build())
            .form_id(FormId::builder().form_id(*f.form_id()).build())
            .build()
    }
}
