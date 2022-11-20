use crate::form::controllers::raw_form::RawForm;
use crate::form::domain::{FormId, FormTitle};

struct Form {
    form_titles: Vec<FormTitle>,
    form_id: FormId,
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
