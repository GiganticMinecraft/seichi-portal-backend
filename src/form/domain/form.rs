use crate::form::domain::{FormId, FormTitle};
use derive_getters::Getters;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Getters)]
pub struct Form {
    pub form_titles: Vec<String>,
    pub form_id: u8,
}

struct DomainForm {
    form_titles: Vec<FormTitle>,
    form_id: FormId,
}

impl From<Form> for DomainForm {
    fn from(f: Form) -> Self {
        DomainForm {
            form_titles: f
                .form_titles()
                .into_iter()
                .map(|t| FormTitle {
                    title: t.to_string(),
                })
                .collect(),
            form_id: FormId { form_id: f.form_id },
        }
    }
}
