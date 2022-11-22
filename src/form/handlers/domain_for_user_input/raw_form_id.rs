use derive_getters::Getters;
use diesel::sql_types::Integer;
use diesel::QueryableByName;
use serde::Deserialize;

#[derive(QueryableByName, Deserialize, Getters)]
pub struct RawFormId {
    #[diesel(sql_type = Integer)]
    id: i32,
}
