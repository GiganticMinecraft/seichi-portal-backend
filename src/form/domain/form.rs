use crate::form::domain::{FormId, FormName};
use derive_getters::Getters;
use typed_builder::TypedBuilder;

#[derive(TypedBuilder, Getters)]
pub struct Form {
    form_name: FormName,
    form_id: FormId,
}
