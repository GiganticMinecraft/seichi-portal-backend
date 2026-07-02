use chrono::{DateTime, Utc};
use domain::account::models::{Role, UserGroupName};
use serde::{Deserialize, Serialize};
use types::non_empty_string::NonEmptyString;
use uuid::Uuid;

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct UserInfoResponse {
    pub id: String,
    pub name: String,
    pub role: String,
    pub groups: Vec<UserGroupSchema>,
    pub discord_user_id: Option<String>,
    pub discord_username: Option<String>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct UserSchema {
    pub id: String,
    pub name: String,
    pub role: String,
    pub groups: Vec<UserGroupSchema>,
}

#[derive(Deserialize, Debug, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct UserListQuery {
    /// Maximum number of users to return
    #[param(minimum = 1, maximum = 100)]
    pub limit: Option<u32>,
    /// Cursor returned by the previous page
    pub cursor: Option<String>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct UserListPageResponse {
    pub items: Vec<UserSchema>,
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct UserGroupSchema {
    pub id: String,
    pub name: String,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct UserGroupRequest {
    #[schema(value_type = String)]
    pub name: UserGroupName,
}

#[derive(Deserialize, Debug, utoipa::ToSchema)]
pub struct AnswerSubmitterRestrictionRequest {
    #[schema(value_type = String)]
    pub reason: NonEmptyString,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct AnswerSubmitterRestrictionResponse {
    pub id: String,
    pub submitter_id: String,
    pub reason: String,
    pub restricted_by: String,
    pub restricted_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct AnswerSubmitterRestrictionHistoryResponse {
    pub id: String,
    pub submitter_id: String,
    pub reason: String,
    pub restricted_by: String,
    pub restricted_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub lifted_at: Option<DateTime<Utc>>,
    pub lifted_by: Option<String>,
}

impl From<domain::form::answer::AnswerSubmitterRestriction> for AnswerSubmitterRestrictionResponse {
    fn from(value: domain::form::answer::AnswerSubmitterRestriction) -> Self {
        Self {
            id: value.id().to_string(),
            submitter_id: value.submitter_id().to_string(),
            reason: value.reason().to_owned().into_inner().into_inner(),
            restricted_by: value.restricted_by().to_string(),
            restricted_at: *value.restricted_at(),
            expires_at: *value.expires_at(),
        }
    }
}

impl From<domain::form::answer::AnswerSubmitterRestriction>
    for AnswerSubmitterRestrictionHistoryResponse
{
    fn from(value: domain::form::answer::AnswerSubmitterRestriction) -> Self {
        Self {
            id: value.id().to_string(),
            submitter_id: value.submitter_id().to_string(),
            reason: value.reason().to_owned().into_inner().into_inner(),
            restricted_by: value.restricted_by().to_string(),
            restricted_at: *value.restricted_at(),
            expires_at: *value.expires_at(),
            lifted_at: *value.lifted_at(),
            lifted_by: value.lifted_by().map(|lifted_by| lifted_by.to_string()),
        }
    }
}

impl From<domain::account::models::AccountUser> for UserSchema {
    fn from(val: domain::account::models::AccountUser) -> Self {
        let groups = val.groups().iter().cloned().map(Into::into).collect();

        UserSchema {
            id: val.id().to_string(),
            name: val.name().to_owned(),
            role: val.role().to_string(),
            groups,
        }
    }
}

impl From<domain::account::models::UserGroup> for UserGroupSchema {
    fn from(value: domain::account::models::UserGroup) -> Self {
        Self {
            id: value.id().to_string(),
            name: value.name().to_string(),
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
