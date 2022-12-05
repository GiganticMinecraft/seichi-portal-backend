use derive_getters::Getters;
use typed_builder::TypedBuilder;

#[derive(TypedBuilder, Getters)]
pub struct FormId {
    form_id: i32,
}
