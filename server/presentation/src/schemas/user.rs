use chrono::{DateTime, Utc};
use domain::account::models::Role;
use serde::{Deserialize, Serialize};
use types::non_empty_string::NonEmptyString;
use uuid::Uuid;

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct UserInfoResponse {
    pub id: String,
    pub name: String,
    pub role: String,
    pub discord_user_id: Option<String>,
    pub discord_username: Option<String>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct UserSchema {
    pub id: String,
    pub name: String,
    pub role: String,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct AnswerSubmissionRestrictionRequest {
    #[schema(value_type = String)]
    pub reason: NonEmptyString,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct AnswerSubmissionRestrictionResponse {
    pub id: String,
    pub user_id: String,
    pub reason: String,
    pub restricted_by: String,
    pub restricted_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl From<domain::account::models::AnswerSubmissionRestriction>
    for AnswerSubmissionRestrictionResponse
{
    fn from(value: domain::account::models::AnswerSubmissionRestriction) -> Self {
        Self {
            id: value.id().to_string(),
            user_id: value.user_id().to_string(),
            reason: value.reason().to_owned().into_inner().into_inner(),
            restricted_by: value.restricted_by().to_string(),
            restricted_at: *value.restricted_at(),
            expires_at: *value.expires_at(),
        }
    }
}

impl From<domain::account::models::AccountUser> for UserSchema {
    fn from(val: domain::account::models::AccountUser) -> Self {
        UserSchema {
            id: val.id().to_string(),
            name: val.name().to_owned(),
            role: val.role().to_string(),
        }
    }
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct DiscordOAuthToken {
    pub token: String,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct UserUpdateSchema {
    pub name: Option<String>,
    pub id: Option<Uuid>,
    #[schema(value_type = Option<String>)]
    pub role: Option<Role>,
}
