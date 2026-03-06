use serde::Deserialize;

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct SessionCreateSchema {
    pub expires: u32,
}
