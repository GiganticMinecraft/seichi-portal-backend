use crate::form::domain::{FormId, FormName, Question};
use derive_getters::Getters;
use typed_builder::TypedBuilder;

#[derive(TypedBuilder, Getters)]
pub struct Form {
    id: FormId,
    name: FormName,
    questions: Vec<Question>,
}
