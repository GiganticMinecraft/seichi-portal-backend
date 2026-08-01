use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::{account::models::UserId, minecraft_ban::MinecraftBan};
use errors::infra::InfraError;
use uuid::Uuid;

use crate::database::{components::MinecraftBanDatabase, connection::ConnectionPool};

struct MinecraftBanRow {
    uuid: Option<String>,
    reason: String,
    punished_at_millis: i64,
    expires_at_millis: i64,
}

#[async_trait]
impl MinecraftBanDatabase for ConnectionPool {
    #[tracing::instrument(skip_all, fields(
        otel.kind = "client",
        db.system = "mariadb",
        db.collection.name = "litebans_bans"
    ))]
    async fn list_by_user_id(&self, user_id: UserId) -> Result<Vec<MinecraftBan>, InfraError> {
        let rows = sqlx::query_as!(
            MinecraftBanRow,
            r#"
            SELECT
                uuid,
                reason,
                `time` AS punished_at_millis,
                `until` AS expires_at_millis
            FROM litebans_bans
            WHERE uuid = ?
            ORDER BY `time` DESC, id DESC
            "#,
            user_id.to_string(),
        )
        .fetch_all(&self.minecraft_bans_pool)
        .await?;

        rows.into_iter().map(minecraft_ban_from_row).collect()
    }
}

fn minecraft_ban_from_row(row: MinecraftBanRow) -> Result<MinecraftBan, InfraError> {
    let uuid = row.uuid.ok_or_else(|| InfraError::Unexpected {
        cause: "LiteBans ban row has no UUID".to_string(),
    })?;
    let punished_at = datetime_from_epoch_millis(row.punished_at_millis, "time")?;
    let expires_at = (row.expires_at_millis > 0)
        .then(|| datetime_from_epoch_millis(row.expires_at_millis, "until"))
        .transpose()?;

    Ok(unsafe {
        MinecraftBan::from_raw_parts(
            UserId::from(Uuid::parse_str(&uuid)?),
            row.reason,
            punished_at,
            expires_at,
        )
    })
}

fn datetime_from_epoch_millis(value: i64, column: &str) -> Result<DateTime<Utc>, InfraError> {
    DateTime::from_timestamp_millis(value).ok_or_else(|| InfraError::Unexpected {
        cause: format!("LiteBans {column} is outside Chrono's supported range"),
    })
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    fn row(expires_at_millis: i64) -> MinecraftBanRow {
        MinecraftBanRow {
            uuid: Some("00000000-0000-0000-0000-000000000001".to_string()),
            reason: "reason".to_string(),
            punished_at_millis: 1_700_000_000_123,
            expires_at_millis,
        }
    }

    #[test]
    fn converts_litebans_epoch_milliseconds_and_permanent_expiries() {
        let temporary = minecraft_ban_from_row(row(1_800_000_000_456)).unwrap();
        let permanent_with_zero = minecraft_ban_from_row(row(0)).unwrap();
        let permanent_with_negative_value = minecraft_ban_from_row(row(-1)).unwrap();

        assert_eq!(
            *temporary.punished_at(),
            Utc.timestamp_millis_opt(1_700_000_000_123)
                .single()
                .unwrap()
        );
        assert_eq!(
            *temporary.expires_at(),
            Some(
                Utc.timestamp_millis_opt(1_800_000_000_456)
                    .single()
                    .unwrap()
            )
        );
        assert_eq!(*permanent_with_zero.expires_at(), None);
        assert_eq!(*permanent_with_negative_value.expires_at(), None);
    }
}
