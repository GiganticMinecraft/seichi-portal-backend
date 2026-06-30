use std::collections::HashMap;
use std::str::FromStr;

use async_trait::async_trait;
use chrono::Utc;
use domain::{
    account::models::{
        AccountUser, DiscordAccountLink, Role, UserGroup, UserGroupId, UserGroupName,
        UserPagePosition,
    },
    form::answer::{
        AnswerSubmitterRestriction, AnswerSubmitterRestrictionId, AnswerSubmitterRestrictionReason,
    },
    pagination::{Page, PageRequest},
};
use errors::infra::InfraError;
use itertools::Itertools;
use redis::Commands;
use sha256::digest;
use sqlx::{AssertSqlSafe, Row, query};
use types::non_empty_string::NonEmptyString;
use uuid::Uuid;

use crate::{
    database::{
        components::{AnswerSubmitterRestrictionDatabase, UserDatabase},
        connection::{ConnectionPool, DatabaseTransaction, redis_connection},
        count::count_as_u32,
    },
    records::DiscordUserRecord,
};

struct UserRow {
    id: String,
    name: String,
    role: String,
}

async fn fetch_groups_by_user_id(
    txn: &mut DatabaseTransaction,
    user_id: Uuid,
) -> Result<Vec<UserGroup>, InfraError> {
    let rows = sqlx::query!(
        r#"SELECT g.id, g.name
        FROM user_groups g
        INNER JOIN user_group_memberships m ON g.id = m.group_id
        WHERE m.user_id = ?
        ORDER BY g.name ASC, g.id ASC"#,
        user_id.to_string(),
    )
    .fetch_all(&mut **txn)
    .await?;

    rows.into_iter()
        .map(|row| user_group_from_parts(row.id, row.name))
        .collect()
}

async fn fetch_groups_by_user_ids(
    txn: &mut DatabaseTransaction,
    user_ids: &[String],
) -> Result<HashMap<String, Vec<UserGroup>>, InfraError> {
    if user_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let sql = format!(
        "SELECT m.user_id, g.id AS group_id, g.name AS group_name
        FROM user_groups g
        INNER JOIN user_group_memberships m ON g.id = m.group_id
        WHERE m.user_id IN ({})
        ORDER BY g.name ASC, g.id ASC",
        std::iter::repeat_n("?", user_ids.len()).join(", ")
    );

    let rows = user_ids
        .iter()
        .fold(query(AssertSqlSafe(&*sql)), |query, uid| query.bind(uid))
        .fetch_all(&mut **txn)
        .await?;

    rows.into_iter()
        .map(|row| {
            let user_id: String = row.try_get("user_id")?;
            let group =
                user_group_from_parts(row.try_get("group_id")?, row.try_get("group_name")?)?;
            Ok::<_, InfraError>((user_id, group))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|pairs| pairs.into_iter().into_group_map())
}

fn user_group_from_parts(id: String, name: String) -> Result<UserGroup, InfraError> {
    let name = NonEmptyString::try_new(name).map_err(|error| InfraError::Unexpected {
        cause: error.to_string(),
    })?;

    Ok(
        unsafe {
            UserGroup::from_raw_parts(Uuid::parse_str(&id)?.into(), UserGroupName::new(name))
        },
    )
}

#[async_trait]
impl UserDatabase for ConnectionPool {
    async fn find_by(&self, uuid: Uuid) -> Result<Option<AccountUser>, InfraError> {
        Ok(self
            .read_only_transaction(|txn| {
                Box::pin(async move {
                    let query = sqlx::query!(
                        "SELECT name, role FROM users WHERE id = ?",
                        uuid.to_string()
                    )
                    .fetch_optional(&mut **txn)
                    .await?;

                    let user = match query {
                        Some(row) => {
                            let groups = fetch_groups_by_user_id(txn, uuid).await?;
                            Some(AccountUser::with_groups(
                                row.name,
                                uuid.into(),
                                Role::from_str(&row.role)?,
                                groups,
                            ))
                        }
                        None => None,
                    };

                    Ok::<_, InfraError>(user)
                })
            })
            .await?)
    }

    async fn find_by_ids(&self, uuids: Vec<Uuid>) -> Result<Vec<AccountUser>, InfraError> {
        if uuids.is_empty() {
            return Ok(Vec::new());
        }

        let uuid_strings = uuids.into_iter().map(|id| id.to_string()).collect_vec();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let sql = format!(
                    "SELECT id, name, role FROM users WHERE id IN ({})",
                    std::iter::repeat_n("?", uuid_strings.len()).join(", ")
                );

                let rows = uuid_strings
                    .iter()
                    .fold(query(AssertSqlSafe(&*sql)), |query, uuid| query.bind(uuid))
                    .fetch_all(&mut **txn)
                    .await?;

                let mut groups_by_user = fetch_groups_by_user_ids(txn, &uuid_strings).await?;

                rows.into_iter()
                    .map(|row| {
                        let id: String = row.try_get("id")?;
                        let name: String = row.try_get("name")?;
                        let role: String = row.try_get("role")?;
                        let user_id = Uuid::parse_str(&id)?;
                        let groups = groups_by_user.remove(&id).unwrap_or_default();
                        Ok::<_, InfraError>(AccountUser::with_groups(
                            name,
                            user_id.into(),
                            Role::from_str(&role)?,
                            groups,
                        ))
                    })
                    .collect()
            })
        })
        .await
    }

    async fn upsert_user(&self, user: &AccountUser) -> Result<(), InfraError> {
        let user_id = user.id().to_string();
        let user_name = user.name().to_owned();
        let user_role = user.role().to_string();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "INSERT INTO users (id, name, role) VALUES (?, ?, ?)
                        ON DUPLICATE KEY UPDATE
                        name = VALUES(name)",
                    user_id,
                    user_name,
                    user_role,
                )
                .execute(&mut **txn)
                .await?;

                Ok::<(), InfraError>(())
            })
        })
        .await
    }

    async fn patch_user_role(&self, uuid: Uuid, role: Role) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "UPDATE users SET role = ? WHERE id = ?",
                    role.to_string(),
                    uuid.to_string(),
                )
                .execute(&mut **txn)
                .await?;

                Ok::<(), InfraError>(())
            })
        })
        .await
    }

    async fn create_user_group(&self, group: &UserGroup) -> Result<(), InfraError> {
        let group_id = group.id().to_string();
        let group_name = group.name().to_string();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "INSERT INTO user_groups (id, name) VALUES (?, ?)",
                    group_id,
                    group_name,
                )
                .execute(&mut **txn)
                .await?;

                Ok::<(), InfraError>(())
            })
        })
        .await
    }

    async fn update_user_group(&self, group: &UserGroup) -> Result<(), InfraError> {
        let group_id = group.id().to_string();
        let group_name = group.name().to_string();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "UPDATE user_groups SET name = ? WHERE id = ?",
                    group_name,
                    group_id,
                )
                .execute(&mut **txn)
                .await?;

                Ok::<(), InfraError>(())
            })
        })
        .await
    }

    async fn delete_user_group(&self, group_id: UserGroupId) -> Result<(), InfraError> {
        let group_id = group_id.to_string();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!("DELETE FROM user_groups WHERE id = ?", group_id)
                    .execute(&mut **txn)
                    .await?;

                Ok::<(), InfraError>(())
            })
        })
        .await
    }

    async fn find_user_group(
        &self,
        group_id: UserGroupId,
    ) -> Result<Option<UserGroup>, InfraError> {
        let group_id = group_id.to_string();

        self.read_only_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!("SELECT id, name FROM user_groups WHERE id = ?", group_id)
                    .fetch_optional(&mut **txn)
                    .await?
                    .map(|row| user_group_from_parts(row.id, row.name))
                    .transpose()
            })
        })
        .await
    }

    async fn fetch_user_groups(&self) -> Result<Vec<UserGroup>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!("SELECT id, name FROM user_groups ORDER BY name ASC, id ASC")
                    .fetch_all(&mut **txn)
                    .await?
                    .into_iter()
                    .map(|row| user_group_from_parts(row.id, row.name))
                    .collect()
            })
        })
        .await
    }

    async fn add_user_to_group(
        &self,
        group_id: UserGroupId,
        user_id: Uuid,
    ) -> Result<(), InfraError> {
        let group_id = group_id.to_string();
        let user_id = user_id.to_string();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    r#"INSERT IGNORE INTO user_group_memberships (group_id, user_id)
                    VALUES (?, ?)"#,
                    group_id,
                    user_id,
                )
                .execute(&mut **txn)
                .await?;

                Ok::<(), InfraError>(())
            })
        })
        .await
    }

    async fn remove_user_from_group(
        &self,
        group_id: UserGroupId,
        user_id: Uuid,
    ) -> Result<(), InfraError> {
        let group_id = group_id.to_string();
        let user_id = user_id.to_string();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "DELETE FROM user_group_memberships WHERE group_id = ? AND user_id = ?",
                    group_id,
                    user_id,
                )
                .execute(&mut **txn)
                .await?;

                Ok::<(), InfraError>(())
            })
        })
        .await
    }

    async fn fetch_all_users(&self) -> Result<Vec<AccountUser>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let rows = sqlx::query!("SELECT id, name, role FROM users")
                    .fetch_all(&mut **txn)
                    .await?;

                let user_ids = rows.iter().map(|row| row.id.clone()).collect_vec();
                let mut groups_by_user = fetch_groups_by_user_ids(txn, &user_ids).await?;

                rows.into_iter()
                    .map(|row| {
                        let user_id = Uuid::parse_str(&row.id)?;
                        let groups = groups_by_user.remove(&row.id).unwrap_or_default();
                        Ok::<_, InfraError>(AccountUser::with_groups(
                            row.name,
                            user_id.into(),
                            Role::from_str(&row.role)?,
                            groups,
                        ))
                    })
                    .collect()
            })
        })
        .await
    }

    async fn fetch_users_page(
        &self,
        request: PageRequest<UserPagePosition>,
    ) -> Result<Page<AccountUser, UserPagePosition>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let limit = i64::from(request.limit().overfetch_value());
                let rows = if let Some(position) = request.after_position() {
                    let after_user_id = position.last_user_id().to_string();
                    sqlx::query_as!(
                        UserRow,
                        r#"SELECT id, name, role
                        FROM users
                        WHERE id > ?
                        ORDER BY id ASC
                        LIMIT ?"#,
                        after_user_id,
                        limit,
                    )
                    .fetch_all(&mut **txn)
                    .await?
                } else {
                    sqlx::query_as!(
                        UserRow,
                        r#"SELECT id, name, role
                        FROM users
                        ORDER BY id ASC
                        LIMIT ?"#,
                        limit,
                    )
                    .fetch_all(&mut **txn)
                    .await?
                };

                let user_ids = rows.iter().map(|row| row.id.clone()).collect_vec();
                let mut groups_by_user = fetch_groups_by_user_ids(txn, &user_ids).await?;
                let users = rows
                    .into_iter()
                    .map(|row| {
                        let user_id = Uuid::parse_str(&row.id)?;
                        let groups = groups_by_user.remove(&row.id).unwrap_or_default();
                        Ok::<_, InfraError>(AccountUser::with_groups(
                            row.name,
                            user_id.into(),
                            Role::from_str(&row.role)?,
                            groups,
                        ))
                    })
                    .collect::<Result<Vec<_>, _>>()?;

                Ok::<_, InfraError>(Page::from_overfetched_items(
                    users,
                    request.limit(),
                    |user| UserPagePosition::new(*user.id()),
                ))
            })
        })
        .await
    }

    async fn start_user_session(
        &self,
        xbox_token: String,
        user: &AccountUser,
        expires: u32,
    ) -> Result<String, InfraError> {
        let now = Utc::now().timestamp_millis();
        let session_id = digest(format!("{xbox_token}{now}"));

        let mut redis_connection = redis_connection().await;

        let user_json = serde_json::to_string(user)?;
        redis_connection.set_ex::<&str, String, ()>(&session_id, user_json, expires as u64)?;

        Ok(session_id)
    }

    async fn fetch_user_by_session_id(
        &self,
        session_id: String,
    ) -> Result<Option<AccountUser>, InfraError> {
        let mut redis_connection = redis_connection().await;

        let result: Option<String> = redis_connection.get(&session_id)?;
        let Some(cached_user) = result.and_then(|s| serde_json::from_str::<AccountUser>(&s).ok())
        else {
            return Ok(None);
        };

        self.find_by(cached_user.id().into_inner()).await
    }

    async fn end_user_session(&self, session_id: String) -> Result<(), InfraError> {
        let mut redis_connection = redis_connection().await;

        redis_connection.del::<&str, ()>(&session_id)?;

        Ok(())
    }

    async fn link_discord_user(&self, link: &DiscordAccountLink) -> Result<(), InfraError> {
        let user_id = link.user_id().to_string();
        let discord_user_id = link.discord_user().id().to_owned().into_inner();
        let discord_username = link
            .discord_user()
            .name()
            .to_owned()
            .into_inner()
            .to_owned();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    r#"INSERT INTO discord_linked_users (user_id, discord_id, discord_username)
                    VALUES (?, ?, ?)
                    ON DUPLICATE KEY UPDATE
                    discord_id = VALUES(discord_id),
                    discord_username = VALUES(discord_username)
                    "#,
                    user_id,
                    discord_user_id,
                    discord_username,
                )
                .execute(&mut **txn)
                .await?;

                Ok::<(), InfraError>(())
            })
        })
        .await
    }

    async fn unlink_discord_user(&self, link: &DiscordAccountLink) -> Result<(), InfraError> {
        let user_id = link.user_id().to_string();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    "DELETE FROM discord_linked_users WHERE user_id = ?",
                    user_id,
                )
                .execute(&mut **txn)
                .await?;

                Ok::<(), InfraError>(())
            })
        })
        .await
    }

    async fn fetch_discord_user(
        &self,
        user: &AccountUser,
    ) -> Result<Option<DiscordUserRecord>, InfraError> {
        let user_id = user.id().to_string();

        Ok(self
            .read_only_transaction(|txn| {
                Box::pin(async move {
                    let query = sqlx::query!(
                        "SELECT discord_id, discord_username FROM discord_linked_users WHERE \
                         user_id = ?",
                        user_id,
                    )
                    .fetch_optional(&mut **txn)
                    .await?;

                    query
                        .map(|row| {
                            Ok::<_, InfraError>(DiscordUserRecord {
                                user_id: row.discord_id,
                                username: row.discord_username,
                            })
                        })
                        .transpose()
                })
            })
            .await?)
    }

    async fn fetch_size(&self) -> Result<u32, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let size = sqlx::query_scalar!("SELECT COUNT(*) AS `count!: i64` FROM users")
                    .fetch_one(&mut **txn)
                    .await?;

                count_as_u32(size, "users")
            })
        })
        .await
    }
}

#[async_trait]
impl AnswerSubmitterRestrictionDatabase for ConnectionPool {
    async fn fetch_active_by_submitter_id(
        &self,
        submitter_id: Uuid,
    ) -> Result<Option<AnswerSubmitterRestriction>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let row = sqlx::query!(
                    r#"
                    SELECT id, submitter_id, reason, restricted_by, restricted_at, expires_at
                    FROM answer_submitter_restrictions
                    WHERE submitter_id = ?
                      AND lifted_at IS NULL
                      AND (expires_at IS NULL OR expires_at > UTC_TIMESTAMP(6))
                    ORDER BY restricted_at DESC
                    LIMIT 1
                    "#,
                    submitter_id.to_string(),
                )
                .fetch_optional(&mut **txn)
                .await?;

                row.map(|row| {
                    Ok::<_, InfraError>(unsafe {
                        AnswerSubmitterRestriction::from_raw_parts(
                            AnswerSubmitterRestrictionId::from(Uuid::parse_str(&row.id)?),
                            Uuid::parse_str(&row.submitter_id)?.into(),
                            AnswerSubmitterRestrictionReason::new(row.reason.try_into().map_err(
                                |err: errors::validation::ValidationError| InfraError::Unexpected {
                                    cause: err.to_string(),
                                },
                            )?),
                            Uuid::parse_str(&row.restricted_by)?.into(),
                            row.restricted_at.and_utc(),
                            row.expires_at.map(|expires_at| expires_at.and_utc()),
                        )
                    })
                })
                .transpose()
            })
        })
        .await
    }

    async fn restrict(&self, restriction: &AnswerSubmitterRestriction) -> Result<(), InfraError> {
        let restriction_id = restriction.id().to_string();
        let submitter_id = restriction.submitter_id().to_string();
        let reason = restriction.reason().to_owned().into_inner().into_inner();
        let restricted_by = restriction.restricted_by().to_string();
        let restricted_at = *restriction.restricted_at();
        let expires_at = *restriction.expires_at();

        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    r#"
                    UPDATE answer_submitter_restrictions
                    SET lifted_at = UTC_TIMESTAMP(6), lifted_by = ?
                    WHERE submitter_id = ?
                      AND lifted_at IS NULL
                      AND (expires_at IS NULL OR expires_at > UTC_TIMESTAMP(6))
                    "#,
                    restricted_by,
                    submitter_id,
                )
                .execute(&mut **txn)
                .await?;

                sqlx::query!(
                    r#"
                    INSERT INTO answer_submitter_restrictions
                        (id, submitter_id, reason, restricted_by, restricted_at, expires_at)
                    VALUES (?, ?, ?, ?, ?, ?)
                    "#,
                    restriction_id,
                    submitter_id,
                    reason,
                    restricted_by,
                    restricted_at,
                    expires_at,
                )
                .execute(&mut **txn)
                .await?;

                Ok::<(), InfraError>(())
            })
        })
        .await
    }

    async fn lift(&self, submitter_id: Uuid, lifted_by: Uuid) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                sqlx::query!(
                    r#"
                    UPDATE answer_submitter_restrictions
                    SET lifted_at = UTC_TIMESTAMP(6), lifted_by = ?
                    WHERE submitter_id = ?
                      AND lifted_at IS NULL
                      AND (expires_at IS NULL OR expires_at > UTC_TIMESTAMP(6))
                    "#,
                    lifted_by.to_string(),
                    submitter_id.to_string(),
                )
                .execute(&mut **txn)
                .await?;

                Ok::<(), InfraError>(())
            })
        })
        .await
    }
}
