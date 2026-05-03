use async_trait::async_trait;
use domain::form::{
    models::FormId,
    question::models::{Question, QuestionId, QuestionType},
};
use errors::infra::InfraError;
use itertools::Itertools;
use sqlx::{MySqlConnection, Row, query};
use std::collections::{BTreeMap, BTreeSet};
use uuid::Uuid;

use crate::{
    database::{components::FormQuestionDatabase, connection::ConnectionPool},
    dto::{ChoiceDto, QuestionDto},
};

#[async_trait]
impl FormQuestionDatabase for ConnectionPool {
    #[tracing::instrument]
    async fn create_questions(
        &self,
        form_id: FormId,
        questions: Vec<Question>,
    ) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                if questions.is_empty() {
                    return Ok::<_, InfraError>(());
                }

                let assigned_questions =
                    insert_new_questions(txn, &form_id, questions.iter().collect_vec()).await?;
                sync_choices_for_questions(txn, &assigned_questions).await
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn put_questions(
        &self,
        form_id: FormId,
        questions: Vec<Question>,
    ) -> Result<(), InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move {
                let form_id_string = form_id.into_inner().to_string();
                let existing_question_ids = fetch_question_ids(txn, &form_id_string).await?;
                let existing_question_id_set = existing_question_ids
                    .iter()
                    .copied()
                    .collect::<BTreeSet<_>>();

                let (existing_questions, new_questions): (Vec<_>, Vec<_>) =
                    questions.iter().partition(|question| {
                        existing_question_id_set.contains(&question.id().into_inner())
                    });

                temporarily_relocate_existing_questions(txn, &form_id_string).await?;
                upsert_existing_questions(txn, &form_id_string, &existing_questions).await?;
                let inserted_questions = insert_new_questions(txn, &form_id, new_questions).await?;

                let retained_question_ids = existing_questions
                    .iter()
                    .map(|question| question.id().into_inner())
                    .chain(
                        inserted_questions
                            .iter()
                            .map(|(question_id, _)| question_id.into_inner()),
                    )
                    .collect::<BTreeSet<_>>();
                delete_questions(
                    txn,
                    existing_question_ids
                        .into_iter()
                        .filter(|question_id| !retained_question_ids.contains(question_id))
                        .collect_vec(),
                )
                .await?;

                let assigned_questions = existing_questions
                    .into_iter()
                    .map(|question| (question.id(), question))
                    .chain(inserted_questions)
                    .collect_vec();

                sync_choices_for_questions(txn, &assigned_questions).await
            })
        })
        .await
    }

    #[tracing::instrument]
    async fn get_questions(&self, form_id: FormId) -> Result<Vec<QuestionDto>, InfraError> {
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                let form_id = form_id.into_inner().to_string();
                let questions_rs = sqlx::query(
                    r"SELECT question_id, form_id, template_key, position, title, description, question_type, is_required
                    FROM form_questions
                    WHERE form_id = ?
                    ORDER BY position ASC, question_id ASC",
                )
                .bind(form_id.clone())
                .fetch_all(&mut **txn)
                .await?;

                let choices_by_question_id = sqlx::query(
                    r"SELECT form_choices.id, form_choices.question_id, form_choices.position, form_choices.label
                    FROM form_choices
                    INNER JOIN form_questions ON form_choices.question_id = form_questions.question_id
                    WHERE form_questions.form_id = ?
                    ORDER BY form_choices.position ASC, form_choices.id ASC",
                )
                .bind(form_id)
                .fetch_all(&mut **txn)
                .await?
                .into_iter()
                .map(|choice_rs| {
                    let question_id = Uuid::parse_str(&choice_rs.try_get::<String, _>("question_id")?)?;
                    Ok::<_, InfraError>((
                        question_id,
                        ChoiceDto {
                            id: Some(choice_rs.try_get("id")?),
                            position: choice_rs.try_get::<u16, _>("position")?,
                            label: choice_rs.try_get("label")?,
                        },
                    ))
                })
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .into_group_map();

                questions_rs
                    .into_iter()
                    .map(|question_rs| {
                        let question_id = Uuid::parse_str(&question_rs.try_get::<String, _>("question_id")?)?;

                        Ok::<_, InfraError>(QuestionDto {
                            id: question_id.to_string(),
                            form_id: question_rs.try_get("form_id")?,
                            template_key: question_rs.try_get("template_key")?,
                            position: question_rs.try_get::<u16, _>("position")?,
                            title: question_rs.try_get("title")?,
                            description: question_rs.try_get("description")?,
                            question_type: question_rs.try_get::<String, _>("question_type")?,
                            choices: choices_by_question_id
                                .get(&question_id)
                                .cloned()
                                .unwrap_or_default(),
                            is_required: question_rs
                                .try_get::<Option<bool>, _>("is_required")?
                                .unwrap_or(false),
                        })
                    })
                    .collect::<Result<Vec<QuestionDto>, _>>()
            })
        })
        .await
    }
}

async fn fetch_question_ids(
    txn: &mut MySqlConnection,
    form_id: &str,
) -> Result<Vec<Uuid>, InfraError> {
    let rows = sqlx::query(
        "SELECT question_id FROM form_questions WHERE form_id = ? ORDER BY position ASC",
    )
    .bind(form_id)
    .fetch_all(&mut *txn)
    .await?;

    rows.into_iter()
        .map(|row| Ok::<_, InfraError>(Uuid::parse_str(&row.try_get::<String, _>("question_id")?)?))
        .collect()
}

#[derive(Debug, Clone)]
struct ExistingQuestionRow {
    question_id: QuestionId,
    template_key: String,
    position: u16,
}

async fn fetch_existing_questions(
    txn: &mut MySqlConnection,
    form_id: &str,
) -> Result<Vec<ExistingQuestionRow>, InfraError> {
    let rows = sqlx::query(
        "SELECT question_id, template_key, position FROM form_questions WHERE form_id = ? ORDER BY position ASC, question_id ASC",
    )
    .bind(form_id)
    .fetch_all(&mut *txn)
    .await?;

    rows.into_iter()
        .map(|row| {
            Ok::<_, InfraError>(ExistingQuestionRow {
                question_id: Uuid::parse_str(&row.try_get::<String, _>("question_id")?)?.into(),
                template_key: row.try_get("template_key")?,
                position: row.try_get("position")?,
            })
        })
        .collect()
}

fn temporary_template_key_prefix(existing_template_keys: &[String]) -> String {
    std::iter::successors(Some("__tmp_form_question__".to_string()), |prefix| {
        Some(format!("{prefix}_"))
    })
    .find(|prefix| {
        existing_template_keys
            .iter()
            .all(|template_key| !template_key.starts_with(prefix))
    })
    .expect("successors yields at least one prefix candidate")
}

async fn temporarily_relocate_existing_questions(
    txn: &mut MySqlConnection,
    form_id: &str,
) -> Result<(), InfraError> {
    let existing_questions = fetch_existing_questions(txn, form_id).await?;
    if existing_questions.is_empty() {
        return Ok(());
    }

    let position_offset =
        u16::try_from(existing_questions.len()).map_err(|_| InfraError::Unexpected {
            cause: "too many questions to relocate temporarily".to_string(),
        })?;
    let temporary_prefix = temporary_template_key_prefix(
        &existing_questions
            .iter()
            .map(|question| question.template_key.clone())
            .collect_vec(),
    );
    existing_questions.iter().try_for_each(|question| {
        question
            .position
            .checked_add(position_offset)
            .ok_or_else(|| InfraError::Unexpected {
                cause: format!(
                    "temporary position overflow for question_id {}",
                    question.question_id.into_inner()
                ),
            })
            .map(|_| ())
    })?;

    sqlx::query(
        "UPDATE form_questions
         SET template_key = CONCAT(?, question_id),
             position = position + ?
         WHERE form_id = ?",
    )
    .bind(temporary_prefix)
    .bind(position_offset)
    .bind(form_id)
    .execute(&mut *txn)
    .await?;

    Ok(())
}

async fn upsert_existing_questions(
    txn: &mut MySqlConnection,
    form_id: &str,
    questions: &[&Question],
) -> Result<(), InfraError> {
    if questions.is_empty() {
        return Ok(());
    }

    let sql = format!(
        r"INSERT INTO form_questions (question_id, form_id, template_key, position, title, description, question_type, is_required)
        VALUES {}
        ON DUPLICATE KEY UPDATE
        template_key = VALUES(template_key),
        position = VALUES(position),
        title = VALUES(title),
        description = VALUES(description),
        question_type = VALUES(question_type),
        is_required = VALUES(is_required)",
        std::iter::repeat_n("(?, ?, ?, ?, ?, ?, ?, ?)", questions.len()).join(", ")
    );

    questions
        .iter()
        .fold(query(&sql), |query, question| {
            query
                .bind(question.id().into_inner().to_string())
                .bind(form_id)
                .bind(question.template_key().to_owned().into_inner())
                .bind(question.position())
                .bind(question.title().to_owned().into_inner())
                .bind(
                    question
                        .description()
                        .cloned()
                        .map(|description| description.into_inner()),
                )
                .bind(question.question_type().to_string())
                .bind(question.is_required())
        })
        .execute(&mut *txn)
        .await?;

    Ok(())
}

async fn insert_new_questions<'a>(
    txn: &mut MySqlConnection,
    form_id: &FormId,
    questions: Vec<&'a Question>,
) -> Result<Vec<(QuestionId, &'a Question)>, InfraError> {
    if questions.is_empty() {
        return Ok(vec![]);
    }

    let sql = format!(
        "INSERT INTO form_questions (question_id, form_id, template_key, position, title, description, question_type, is_required) VALUES {}",
        std::iter::repeat_n("(?, ?, ?, ?, ?, ?, ?, ?)", questions.len()).join(", ")
    );
    questions
        .iter()
        .fold(query(&sql), |query, question| {
            query
                .bind(question.id().into_inner().to_string())
                .bind(form_id.to_owned().into_inner().to_string())
                .bind(question.template_key().to_owned().into_inner())
                .bind(question.position())
                .bind(question.title().to_owned().into_inner())
                .bind(
                    question
                        .description()
                        .cloned()
                        .map(|description| description.into_inner()),
                )
                .bind(question.question_type().to_string())
                .bind(question.is_required())
        })
        .execute(&mut *txn)
        .await?;

    Ok(questions
        .into_iter()
        .map(|question| (question.id(), question))
        .collect())
}

async fn delete_questions(
    txn: &mut MySqlConnection,
    question_ids: Vec<Uuid>,
) -> Result<(), InfraError> {
    if question_ids.is_empty() {
        return Ok(());
    }

    let sql = format!(
        "DELETE FROM form_questions WHERE question_id IN ({})",
        std::iter::repeat_n("?", question_ids.len()).join(", ")
    );
    question_ids
        .iter()
        .fold(query(&sql), |query, question_id| {
            query.bind(question_id.to_string())
        })
        .execute(&mut *txn)
        .await?;

    Ok(())
}

async fn sync_choices_for_questions(
    txn: &mut MySqlConnection,
    assigned_questions: &[(QuestionId, &Question)],
) -> Result<(), InfraError> {
    let question_ids = assigned_questions
        .iter()
        .map(|(question_id, _)| *question_id)
        .collect_vec();
    if question_ids.is_empty() {
        return Ok(());
    }

    let existing_choice_by_id = fetch_existing_choices(txn, &question_ids).await?;
    let retained_choice_ids = assigned_questions
        .iter()
        .flat_map(|(_, question)| {
            question.choices().into_iter().flat_map(|choices| {
                choices
                    .iter()
                    .filter_map(|choice| choice.id.map(|id| id.into_inner()))
            })
        })
        .collect::<BTreeSet<_>>();

    delete_choices(
        txn,
        existing_choice_by_id
            .keys()
            .filter(|choice_id| !retained_choice_ids.contains(choice_id))
            .copied()
            .collect_vec(),
    )
    .await?;

    let existing_choices = assigned_questions
        .iter()
        .flat_map(|(question_id, question)| {
            question.choices().into_iter().flat_map(|choices| {
                choices.iter().filter_map(|choice| {
                    choice.id.map(|id| {
                        (
                            id.into_inner(),
                            *question_id,
                            choice.position,
                            choice.label.to_owned().into_inner(),
                        )
                    })
                })
            })
        })
        .collect_vec();
    upsert_existing_choices(txn, existing_choices).await?;

    let new_choices = assigned_questions
        .iter()
        .filter(|(_, question)| question.question_type() != QuestionType::Text)
        .flat_map(|(question_id, question)| {
            question.choices().into_iter().flat_map(|choices| {
                choices
                    .iter()
                    .filter(|choice| choice.id.is_none())
                    .map(|choice| {
                        (
                            *question_id,
                            choice.position,
                            choice.label.to_owned().into_inner(),
                        )
                    })
            })
        })
        .collect_vec();
    insert_new_choices(txn, new_choices).await
}

async fn fetch_existing_choices(
    txn: &mut MySqlConnection,
    question_ids: &[QuestionId],
) -> Result<BTreeMap<i32, QuestionId>, InfraError> {
    let sql = format!(
        "SELECT id, question_id FROM form_choices WHERE question_id IN ({})",
        std::iter::repeat_n("?", question_ids.len()).join(", ")
    );

    let rows = question_ids
        .iter()
        .fold(sqlx::query(&sql), |query, question_id| {
            query.bind(question_id.into_inner().to_string())
        })
        .fetch_all(&mut *txn)
        .await?;

    rows.into_iter()
        .map(|row| {
            Ok::<_, InfraError>((
                row.try_get("id")?,
                Uuid::parse_str(&row.try_get::<String, _>("question_id")?)?.into(),
            ))
        })
        .collect()
}

async fn delete_choices(txn: &mut MySqlConnection, choice_ids: Vec<i32>) -> Result<(), InfraError> {
    if choice_ids.is_empty() {
        return Ok(());
    }

    let sql = format!(
        "DELETE FROM form_choices WHERE id IN ({})",
        std::iter::repeat_n("?", choice_ids.len()).join(", ")
    );
    choice_ids
        .iter()
        .fold(query(&sql), |query, choice_id| query.bind(choice_id))
        .execute(&mut *txn)
        .await?;

    Ok(())
}

async fn upsert_existing_choices(
    txn: &mut MySqlConnection,
    choices: Vec<(i32, QuestionId, u16, String)>,
) -> Result<(), InfraError> {
    if choices.is_empty() {
        return Ok(());
    }

    let sql = format!(
        r"INSERT INTO form_choices (id, question_id, position, label) VALUES {}
        ON DUPLICATE KEY UPDATE
        question_id = VALUES(question_id),
        position = VALUES(position),
        label = VALUES(label)",
        std::iter::repeat_n("(?, ?, ?, ?)", choices.len()).join(", ")
    );
    choices
        .iter()
        .fold(
            query(&sql),
            |query, (choice_id, question_id, position, label)| {
                query
                    .bind(choice_id)
                    .bind(question_id.into_inner().to_string())
                    .bind(position)
                    .bind(label)
            },
        )
        .execute(&mut *txn)
        .await?;

    Ok(())
}

async fn insert_new_choices(
    txn: &mut MySqlConnection,
    choices: Vec<(QuestionId, u16, String)>,
) -> Result<(), InfraError> {
    if choices.is_empty() {
        return Ok(());
    }

    let sql = format!(
        "INSERT INTO form_choices (question_id, position, label) VALUES {}",
        std::iter::repeat_n("(?, ?, ?)", choices.len()).join(", ")
    );
    choices
        .iter()
        .fold(query(&sql), |query, (question_id, position, label)| {
            query
                .bind(question_id.into_inner().to_string())
                .bind(position)
                .bind(label)
        })
        .execute(&mut *txn)
        .await?;

    Ok(())
}
