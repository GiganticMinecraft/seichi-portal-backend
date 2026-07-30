use std::collections::HashSet;
use std::str::FromStr;

use async_trait::async_trait;
use domain::form::{
    answer::{AnswerId, AnswerRelation},
    models::FormId,
};
use errors::infra::InfraError;
use itertools::Itertools;
use sqlx::{AssertSqlSafe, Row, query};
use uuid::Uuid;

use crate::database::{
    components::{FormAnswerRelationDatabase, RelatedAnswerRecord},
    connection::{ConnectionPool, DatabaseTransaction},
};

fn normalized_relations(
    answer_id: AnswerId,
    related_answer_ids: Vec<AnswerId>,
) -> Result<Vec<AnswerRelation>, InfraError> {
    let relations = related_answer_ids
        .into_iter()
        .map(|related_answer_id| AnswerRelation::new(answer_id, related_answer_id))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| InfraError::Unexpected {
            cause: error.to_string(),
        })?;
    let unique = relations.iter().copied().collect::<HashSet<_>>();
    if unique.len() != relations.len() {
        return Err(InfraError::Unexpected {
            cause: "duplicate related answer ids are not allowed".to_string(),
        });
    }
    Ok(relations)
}

async fn fetch_answer_identities(
    txn: &mut DatabaseTransaction,
    answer_ids: &[String],
    for_update: bool,
) -> Result<HashSet<String>, InfraError> {
    let sql = format!(
        "SELECT answer_id FROM answer_identities WHERE answer_id IN ({}) ORDER BY answer_id{}",
        std::iter::repeat_n("?", answer_ids.len()).join(", "),
        if for_update { " FOR UPDATE" } else { "" },
    );

    Ok(answer_ids
        .iter()
        .fold(query(AssertSqlSafe(&*sql)), |query, answer_id| {
            query.bind(answer_id)
        })
        .fetch_all(&mut **txn)
        .await?
        .into_iter()
        .map(|row| row.try_get("answer_id"))
        .collect::<Result<_, sqlx::Error>>()?)
}

async fn fetch_active_answer_ids(
    txn: &mut DatabaseTransaction,
    answer_ids: &[String],
    for_update: bool,
) -> Result<HashSet<String>, InfraError> {
    let sql = format!(
        "SELECT id FROM answers WHERE id IN ({}) ORDER BY id{}",
        std::iter::repeat_n("?", answer_ids.len()).join(", "),
        if for_update { " FOR UPDATE" } else { "" },
    );

    Ok(answer_ids
        .iter()
        .fold(query(AssertSqlSafe(&*sql)), |query, answer_id| {
            query.bind(answer_id)
        })
        .fetch_all(&mut **txn)
        .await?
        .into_iter()
        .map(|row| row.try_get("id"))
        .collect::<Result<_, sqlx::Error>>()?)
}

async fn ensure_active_relation_participants(
    txn: &mut DatabaseTransaction,
    answer_id: AnswerId,
    answer_id_string: &str,
    target_ids: &[String],
    locked_ids: &[String],
    for_update: bool,
) -> Result<(), InfraError> {
    let identity_ids = fetch_answer_identities(txn, locked_ids, for_update).await?;
    if !identity_ids.contains(answer_id_string) {
        return Err(InfraError::AnswerRelationSourceNotActive {
            id: answer_id.into_inner(),
        });
    }
    if let Some(target_id) = target_ids
        .iter()
        .find(|target_id| !identity_ids.contains(*target_id))
    {
        return Err(InfraError::AnswerRelationTargetNotActive {
            id: Uuid::from_str(target_id)?,
        });
    }

    let active_ids = fetch_active_answer_ids(txn, locked_ids, for_update).await?;
    if !active_ids.contains(answer_id_string) {
        return Err(InfraError::AnswerRelationSourceNotActive {
            id: answer_id.into_inner(),
        });
    }
    if let Some(target_id) = target_ids
        .iter()
        .find(|target_id| !active_ids.contains(*target_id))
    {
        return Err(InfraError::AnswerRelationTargetNotActive {
            id: Uuid::from_str(target_id)?,
        });
    }

    Ok(())
}

struct RelationParticipants {
    relations: Vec<AnswerRelation>,
    answer_id_string: String,
    target_ids: Vec<String>,
    locked_ids: Vec<String>,
}

fn relation_participants(
    answer_id: AnswerId,
    related_answer_ids: Vec<AnswerId>,
) -> Result<RelationParticipants, InfraError> {
    let relations = normalized_relations(answer_id, related_answer_ids)?;
    let answer_id_string = answer_id.to_string();
    let target_ids = relations
        .iter()
        .filter_map(|relation| relation.other_endpoint(answer_id))
        .map(|id| id.to_string())
        .collect::<Vec<_>>();
    let mut locked_ids = std::iter::once(answer_id_string.clone())
        .chain(target_ids.iter().cloned())
        .collect::<Vec<_>>();
    locked_ids.sort();

    Ok(RelationParticipants {
        relations,
        answer_id_string,
        target_ids,
        locked_ids,
    })
}

async fn replace_relations_in_transaction(
    txn: &mut DatabaseTransaction,
    answer_id_string: &str,
    relations: &[AnswerRelation],
) -> Result<(), InfraError> {
    sqlx::query!(
        "DELETE FROM answer_relations WHERE answer_id_first = ? OR answer_id_second = ?",
        answer_id_string,
        answer_id_string,
    )
    .execute(&mut **txn)
    .await?;

    if !relations.is_empty() {
        // 行数で VALUES の placeholder が変わるため、AssertSqlSafe で動的 SQL を使う。
        let sql = format!(
            "INSERT INTO answer_relations (answer_id_first, answer_id_second) VALUES {}",
            std::iter::repeat_n("(?, ?)", relations.len()).join(", ")
        );
        relations
            .iter()
            .flat_map(|relation| relation.endpoints())
            .fold(query(AssertSqlSafe(&*sql)), |query, endpoint| {
                query.bind(endpoint.to_string())
            })
            .execute(&mut **txn)
            .await?;
    }

    Ok(())
}

#[async_trait]
impl FormAnswerRelationDatabase for ConnectionPool {
    async fn validate_answer_relation_replacement(
        &self,
        answer_id: AnswerId,
        related_answer_ids: &[AnswerId],
    ) -> Result<(), InfraError> {
        let RelationParticipants {
            answer_id_string,
            target_ids,
            locked_ids,
            ..
        } = relation_participants(answer_id, related_answer_ids.to_vec())?;

        self.read_only_transaction(move |txn| {
            Box::pin(async move {
                ensure_active_relation_participants(
                    txn,
                    answer_id,
                    &answer_id_string,
                    &target_ids,
                    &locked_ids,
                    false,
                )
                .await
            })
        })
        .await
    }

    async fn replace_answer_relations(
        &self,
        answer_id: AnswerId,
        related_answer_ids: Vec<AnswerId>,
    ) -> Result<(), InfraError> {
        let RelationParticipants {
            relations,
            answer_id_string,
            target_ids,
            locked_ids,
        } = relation_participants(answer_id, related_answer_ids)?;

        self.read_write_transaction(move |txn| {
            Box::pin(async move {
                ensure_active_relation_participants(
                    txn,
                    answer_id,
                    &answer_id_string,
                    &target_ids,
                    &locked_ids,
                    true,
                )
                .await?;

                replace_relations_in_transaction(txn, &answer_id_string, &relations).await
            })
        })
        .await
    }

    async fn update_answer_meta_and_replace_relations(
        &self,
        answer: &domain::form::answer::AnswerEntry,
        related_answer_ids: Vec<AnswerId>,
    ) -> Result<(), InfraError> {
        let answer = answer.clone();
        let answer_id = *answer.id();
        let RelationParticipants {
            relations,
            answer_id_string,
            target_ids,
            locked_ids,
        } = relation_participants(answer_id, related_answer_ids)?;
        let title = answer
            .title()
            .to_owned()
            .into_inner()
            .map(|title| title.into_inner());
        let publication = answer.publication().to_string();
        let form_id = answer.form_id().to_string();

        self.read_write_transaction(move |txn| {
            Box::pin(async move {
                ensure_active_relation_participants(
                    txn,
                    answer_id,
                    &answer_id_string,
                    &target_ids,
                    &locked_ids,
                    true,
                )
                .await?;

                sqlx::query!(
                    "UPDATE answers SET title = ?, publication = ? WHERE id = ? AND form_id = ?",
                    title,
                    publication,
                    &answer_id_string,
                    form_id,
                )
                .execute(&mut **txn)
                .await?;

                replace_relations_in_transaction(txn, &answer_id_string, &relations).await
            })
        })
        .await
    }

    async fn get_answer_relations(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<RelatedAnswerRecord>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let answer_id = answer_id.to_string();
                let rows = sqlx::query!(
                    r"SELECT identity_row.form_id, identity_row.answer_id,
                        active_answer.id IS NULL AS `is_archived!: bool`
                    FROM answer_relations relation
                    INNER JOIN answer_identities identity_row ON identity_row.answer_id =
                        CASE WHEN relation.answer_id_first = ? THEN relation.answer_id_second ELSE relation.answer_id_first END
                    LEFT JOIN answers active_answer ON active_answer.id = identity_row.answer_id
                    WHERE relation.answer_id_first = ? OR relation.answer_id_second = ?
                    ORDER BY identity_row.answer_id",
                    &answer_id,
                    &answer_id,
                    &answer_id,
                )
                .fetch_all(&mut **txn)
                .await?;

                rows.into_iter()
                    .map(|row| {
                        Ok::<_, InfraError>(RelatedAnswerRecord {
                            form_id: FormId::from(Uuid::from_str(&row.form_id)?),
                            answer_id: AnswerId::from(Uuid::from_str(&row.answer_id)?),
                            is_archived: row.is_archived,
                        })
                    })
                    .collect()
            })
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer_id(value: u128) -> AnswerId {
        AnswerId::from(Uuid::from_u128(value))
    }

    #[test]
    fn replacement_input_rejects_self_and_duplicate_targets() {
        let source = answer_id(1);
        let target = answer_id(2);

        assert!(normalized_relations(source, vec![source]).is_err());
        assert!(normalized_relations(source, vec![target, target]).is_err());
        assert_eq!(normalized_relations(source, vec![target]).unwrap().len(), 1);
    }
}
