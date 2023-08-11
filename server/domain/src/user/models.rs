use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct User {
    pub name: String,
    pub uuid: Uuid,
}
