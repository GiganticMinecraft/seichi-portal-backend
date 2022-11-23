use derive_getters::Getters;
use serde::Deserialize;

#[derive(Deserialize, Getters)]
pub struct RawFormId {
    id: i32,
}
