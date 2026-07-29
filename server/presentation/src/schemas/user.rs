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
pub struct FormSubmissionRestrictionRequest {
    #[schema(value_type = String)]
    pub reason: NonEmptyString,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct FormSubmissionRestrictionResponse {
    pub id: String,
    pub submitter_id: String,
    pub reason: String,
    pub restricted_by: String,
    pub restricted_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct FormSubmissionRestrictionHistoryResponse {
    pub id: String,
    pub submitter_id: String,
    pub reason: String,
    pub restricted_by: String,
    pub restricted_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub lifted_at: Option<DateTime<Utc>>,
    pub lifted_by: Option<String>,
}

#[derive(Serialize, Debug, utoipa::ToSchema)]
pub struct MinecraftPunishmentResponse {
    pub uuid: String,
    pub reason: String,
    pub punished_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl From<domain::minecraft_ban::MinecraftBan> for MinecraftPunishmentResponse {
    fn from(value: domain::minecraft_ban::MinecraftBan) -> Self {
        Self {
            uuid: value.user_id().to_string(),
            reason: value.reason().to_owned(),
            punished_at: *value.punished_at(),
            expires_at: *value.expires_at(),
        }
    }
}

impl From<domain::form::FormSubmissionRestriction> for FormSubmissionRestrictionResponse {
    fn from(value: domain::form::FormSubmissionRestriction) -> Self {
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

impl From<domain::form::FormSubmissionRestriction> for FormSubmissionRestrictionHistoryResponse {
    fn from(value: domain::form::FormSubmissionRestriction) -> Self {
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

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use domain::{account::models::UserId, minecraft_ban::MinecraftBan};
    use serde_json::json;
    use uuid::Uuid;

    use super::*;

    fn ban(until: i64) -> MinecraftBan {
        let punished_at = Utc
            .timestamp_millis_opt(1_700_000_000_123)
            .single()
            .unwrap();
        let expires_at = (until > 0).then(|| Utc.timestamp_millis_opt(until).single().unwrap());

        unsafe {
            MinecraftBan::from_raw_parts(
                UserId::from(Uuid::from_u128(1)),
                "reason".to_string(),
                punished_at,
                expires_at,
            )
        }
    }

    #[test]
    fn minecraft_punishment_serializes_only_public_fields_and_nullable_expiry() {
        let positive =
            serde_json::to_value(MinecraftPunishmentResponse::from(ban(1_800_000_000_456)))
                .unwrap();
        let zero = serde_json::to_value(MinecraftPunishmentResponse::from(ban(0))).unwrap();
        let negative = serde_json::to_value(MinecraftPunishmentResponse::from(ban(-1))).unwrap();

        assert_eq!(
            positive,
            json!({
                "uuid": "00000000-0000-0000-0000-000000000001",
                "reason": "reason",
                "punished_at": "2023-11-14T22:13:20.123Z",
                "expires_at": "2027-01-15T08:00:00.456Z",
            })
        );
        assert_eq!(zero["expires_at"], serde_json::Value::Null);
        assert_eq!(negative["expires_at"], serde_json::Value::Null);
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
