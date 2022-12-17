use derive_getters::Getters;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(Serialize, Deserialize, Getters, TypedBuilder)]
pub struct RawFormId {
    id: i32,
}
