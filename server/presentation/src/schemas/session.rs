use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct SessionCreateSchema {
    pub expires_at: DateTime<Utc>,
}
