use crate::form::domain::{FormId, FormTitle};
use typed_builder::TypedBuilder;

#[derive(TypedBuilder)]
pub struct Form {
    form_titles: Vec<FormTitle>,
    form_id: FormId,
}
