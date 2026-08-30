use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use domain::form::{
    answer::{AnswerAuthor, AnswerLabelId, AnswerReference, AnswerRelation},
    question::QuestionSet,
    redmine_import::{
        RedmineImportAnswerRelationsResult, RedmineImportResult, RedmineImportTarget,
        RedmineImportVerification, RedmineImportedIssue, RedmineIssueRelationBatch,
    },
};
use errors::infra::InfraError;
use types::non_empty_vec::NonEmptyVec;
use uuid::Uuid;

use crate::{
    database::connection::{DatabaseTransaction, RedmineImportConnectionPool},
    records::{ChoiceRecord, QuestionRecord},
};

/// Redmine 移行 Repository が使うデータベース操作です。
#[async_trait]
pub trait RedmineImportDatabase: Send + Sync {
    async fn find_target(
        &self,
        form_id: domain::form::models::FormId,
        form_title: &str,
        label_names: &[String],
    ) -> Result<Option<RedmineImportTarget>, InfraError>;

    async fn verify_issue(
        &self,
        issue: &RedmineImportedIssue,
    ) -> Result<RedmineImportVerification, InfraError>;

    async fn import_issue(
        &self,
        issue: RedmineImportedIssue,
    ) -> Result<RedmineImportResult, InfraError>;

    async fn import_answer_relations(
        &self,
        relations: RedmineIssueRelationBatch,
    ) -> Result<RedmineImportAnswerRelationsResult, InfraError>;
}

#[async_trait]
impl RedmineImportDatabase for RedmineImportConnectionPool {
    async fn find_target(
        &self,
        form_id: domain::form::models::FormId,
        form_title: &str,
        label_names: &[String],
    ) -> Result<Option<RedmineImportTarget>, InfraError> {
        let form_id = form_id.into_inner().to_string();
        let form_title = form_title.to_owned();
        let label_names = label_names.to_vec();
        self.read_only_transaction(|txn| {
            Box::pin(async move {
                find_target_in_transaction(txn, &form_id, &form_title, &label_names).await
            })
        })
        .await
    }

    async fn verify_issue(
        &self,
        issue: &RedmineImportedIssue,
    ) -> Result<RedmineImportVerification, InfraError> {
        let issue = issue.clone();
        self.read_only_transaction(|txn| {
            Box::pin(async move { verify_issue_in_transaction(txn, &issue, false).await })
        })
        .await
    }

    async fn import_issue(
        &self,
        issue: RedmineImportedIssue,
    ) -> Result<RedmineImportResult, InfraError> {
        self.read_write_transaction(|txn| {
            Box::pin(async move { import_issue_in_transaction(txn, issue).await })
        })
        .await
    }

    async fn import_answer_relations(
        &self,
        relations: RedmineIssueRelationBatch,
    ) -> Result<RedmineImportAnswerRelationsResult, InfraError> {
        if relations.relations().is_empty() {
            return Ok(RedmineImportAnswerRelationsResult::default());
        }

        self.read_write_transaction(|txn| {
            Box::pin(async move { import_answer_relations_in_transaction(txn, relations).await })
        })
        .await
    }
}

async fn find_target_in_transaction(
    txn: &mut DatabaseTransaction,
    form_id: &str,
    form_title: &str,
    label_names: &[String],
) -> Result<Option<RedmineImportTarget>, InfraError> {
    let Some(form_row) =
        sqlx::query!("SELECT id, title FROM form_meta_data WHERE id = ?", form_id,)
            .fetch_optional(&mut **txn)
            .await?
    else {
        return Ok(None);
    };
    if form_row.title != form_title {
        return Ok(None);
    }

    let form_id = FormIdParts::parse(&form_row.id)?;
    let question_rows = sqlx::query!(
        r"SELECT question_id, form_id, template_key, position, title, description,
            question_type, is_required
        FROM form_questions
        WHERE form_id = ?
        ORDER BY position ASC, question_id ASC",
        &form_row.id,
    )
    .fetch_all(&mut **txn)
    .await?;
    let choice_rows = sqlx::query!(
        r"SELECT c.id, c.question_id, c.position, c.label
        FROM form_choices c
        INNER JOIN form_questions q ON c.question_id = q.question_id
        WHERE q.form_id = ?
        ORDER BY c.position ASC, c.id ASC",
        &form_row.id,
    )
    .fetch_all(&mut **txn)
    .await?;

    let choices_by_question_id = choice_rows.into_iter().fold(
        HashMap::<String, Vec<ChoiceRecord>>::new(),
        |mut choices, row| {
            choices
                .entry(row.question_id)
                .or_default()
                .push(ChoiceRecord {
                    id: Some(row.id),
                    position: row.position,
                    label: row.label,
                });
            choices
        },
    );
    let question_records = question_rows
        .into_iter()
        .map(|row| {
            Ok::<_, InfraError>(QuestionRecord {
                id: row.question_id.clone(),
                form_id: row.form_id,
                template_key: row.template_key,
                position: row.position,
                title: row.title,
                description: row.description,
                question_type: row.question_type,
                choices: choices_by_question_id
                    .get(&row.question_id)
                    .cloned()
                    .unwrap_or_default(),
                is_required: row.is_required.unwrap_or_default() != 0,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let questions = question_records
        .into_iter()
        .map(|record| {
            record
                .try_into()
                .map_err(|error: errors::Error| InfraError::Unexpected {
                    cause: format!("invalid form question in import target: {error}"),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let questions = NonEmptyVec::try_new(questions).map_err(|error| InfraError::Unexpected {
        cause: format!("import target form has no questions: {error}"),
    })?;
    let questions = QuestionSet::try_new(questions).map_err(|error| InfraError::Unexpected {
        cause: format!("invalid import target questions: {error}"),
    })?;

    let mut label_ids = Vec::with_capacity(label_names.len());
    let mut seen_label_names = HashSet::with_capacity(label_names.len());
    for label_name in label_names {
        if !seen_label_names.insert(label_name) {
            return Err(InfraError::Unexpected {
                cause: format!("duplicate answer label mapping: {label_name:?}"),
            });
        }

        let rows = sqlx::query!(
            "SELECT id, name FROM label_for_form_answers WHERE name = ? ORDER BY id",
            label_name,
        )
        .fetch_all(&mut **txn)
        .await?;
        let exact_rows = rows
            .into_iter()
            .filter(|row| row.name == *label_name)
            .collect::<Vec<_>>();
        let Some(row) = exact_rows.first() else {
            return Err(InfraError::Unexpected {
                cause: format!("answer label does not exist: {label_name:?}"),
            });
        };
        if exact_rows.len() > 1 {
            return Err(InfraError::Unexpected {
                cause: format!("multiple answer labels have name {label_name:?}"),
            });
        }
        label_ids.push(parse_answer_label_id(&row.id)?);
    }

    RedmineImportTarget::try_new(form_id.0, questions, label_ids)
        .map(Some)
        .map_err(|error| InfraError::Unexpected {
            cause: format!("invalid Redmine import target: {error}"),
        })
}

#[derive(Clone, Copy)]
struct FormIdParts(domain::form::models::FormId);

impl FormIdParts {
    fn parse(value: &str) -> Result<Self, InfraError> {
        Ok(Self(domain::form::models::FormId::from(Uuid::from_str(
            value,
        )?)))
    }
}

fn parse_answer_label_id(value: &str) -> Result<AnswerLabelId, InfraError> {
    Ok(AnswerLabelId::from(Uuid::from_str(value)?))
}

fn relation_columns(relation: AnswerRelation) -> (String, String, String, String) {
    let [first, second] = relation.endpoints();
    (
        first.form_id().to_string(),
        first.answer_id().to_string(),
        second.form_id().to_string(),
        second.answer_id().to_string(),
    )
}

async fn resolve_answer_reference_in_transaction(
    txn: &mut DatabaseTransaction,
    issue_id: domain::form::answer::RedmineIssueId,
) -> Result<AnswerReference, InfraError> {
    let issue_id_value = issue_id.into_inner();
    let rows = sqlx::query!(
        r"SELECT reference.answer_id,
                active_answer.form_id AS `active_form_id?: String`,
                archived_answer.form_id AS `archived_form_id?: String`
        FROM redmine_imported_answer_references reference
        LEFT JOIN answers active_answer ON active_answer.id = reference.answer_id
        LEFT JOIN archived_answers archived_answer ON archived_answer.id = reference.answer_id
        WHERE reference.redmine_issue_id = ?
        FOR UPDATE",
        issue_id_value,
    )
    .fetch_all(&mut **txn)
    .await?;

    if rows.is_empty() {
        return Err(InfraError::Unexpected {
            cause: format!("Redmine issue {issue_id_value} has no imported answer reference"),
        });
    }
    if rows.len() > 1 {
        return Err(InfraError::Unexpected {
            cause: format!(
                "Redmine issue {issue_id_value} has duplicate imported answer references"
            ),
        });
    }

    let row = rows
        .into_iter()
        .next()
        .expect("rows was checked to be non-empty");
    let answer_id = Uuid::parse_str(&row.answer_id).map_err(|error| InfraError::Unexpected {
        cause: format!("Redmine issue {issue_id_value} has an invalid answer UUID: {error}"),
    })?;
    let form_id = match (row.active_form_id, row.archived_form_id) {
        (Some(_), Some(_)) => {
            return Err(InfraError::Unexpected {
                cause: format!(
                    "Redmine issue {issue_id_value} points to both active and archived answers"
                ),
            });
        }
        (Some(form_id), None) | (None, Some(form_id)) => form_id,
        (None, None) => {
            return Err(InfraError::Unexpected {
                cause: format!(
                    "Redmine issue {issue_id_value} points to a missing active or archived answer"
                ),
            });
        }
    };
    let form_id = Uuid::parse_str(&form_id).map_err(|error| InfraError::Unexpected {
        cause: format!("Redmine issue {issue_id_value} has an invalid form UUID: {error}"),
    })?;

    Ok(AnswerReference::new(form_id.into(), answer_id.into()))
}

async fn import_answer_relations_in_transaction(
    txn: &mut DatabaseTransaction,
    relations: RedmineIssueRelationBatch,
) -> Result<RedmineImportAnswerRelationsResult, InfraError> {
    let mut inserted = 0;
    let mut already_exists = 0;

    for issue_relation in relations.relations() {
        let first = resolve_answer_reference_in_transaction(txn, issue_relation.first()).await?;
        let second = resolve_answer_reference_in_transaction(txn, issue_relation.second()).await?;
        let relation =
            AnswerRelation::new(first, second).map_err(|error| InfraError::Unexpected {
                cause: format!("invalid answer relation for Redmine issue pair: {error}"),
            })?;
        let (first_form_id, first_answer_id, second_form_id, second_answer_id) =
            relation_columns(relation);

        let existing = sqlx::query!(
            r"SELECT first_form_id
            FROM answer_relations
            WHERE first_form_id = ? AND first_answer_id = ?
              AND second_form_id = ? AND second_answer_id = ?
            FOR UPDATE",
            &first_form_id,
            &first_answer_id,
            &second_form_id,
            &second_answer_id,
        )
        .fetch_optional(&mut **txn)
        .await?;

        sqlx::query!(
            r"INSERT INTO answer_relations
                (first_form_id, first_answer_id, second_form_id, second_answer_id)
            VALUES (?, ?, ?, ?)
            ON DUPLICATE KEY UPDATE first_form_id = first_form_id",
            first_form_id,
            first_answer_id,
            second_form_id,
            second_answer_id,
        )
        .execute(&mut **txn)
        .await?;

        if existing.is_some() {
            already_exists += 1;
        } else {
            inserted += 1;
        }
    }

    Ok(RedmineImportAnswerRelationsResult::new(
        inserted,
        already_exists,
    ))
}

#[derive(Debug, PartialEq)]
struct ImportedJournalPayload {
    journal_id: i64,
    redmine_user_id: Option<i64>,
    author_name: String,
    content: String,
    timestamp: DateTime<Utc>,
}

#[derive(Debug, PartialEq)]
struct ImportedPayload {
    form_id: String,
    redmine_user_id: Option<i64>,
    author_name: String,
    title: Option<String>,
    publication: String,
    status: String,
    timestamp: DateTime<Utc>,
    contents: Vec<(String, String)>,
    journals: Vec<ImportedJournalPayload>,
    label_ids: Vec<String>,
}

fn candidate_payload(issue: &RedmineImportedIssue) -> Result<ImportedPayload, InfraError> {
    let answer = issue.answer();
    let AnswerAuthor::ImportedFromRedmine(author) = answer.author() else {
        return Err(InfraError::Unexpected {
            cause: "Redmine import aggregate has a non-imported answer author".to_string(),
        });
    };
    let title = answer
        .title()
        .clone()
        .into_inner()
        .map(|title| title.into_inner());
    let mut contents = answer
        .contents()
        .iter()
        .map(|content| (content.question_id.to_string(), content.answer.clone()))
        .collect::<Vec<_>>();
    contents.sort_by(|left, right| left.0.cmp(&right.0));

    let mut journals = issue
        .comments()
        .iter()
        .map(|comment| {
            let author = comment
                .redmine_author()
                .ok_or_else(|| InfraError::Unexpected {
                    cause: "Redmine import journal has no author".to_string(),
                })?;
            Ok::<_, InfraError>(ImportedJournalPayload {
                journal_id: comment
                    .redmine_journal_id()
                    .ok_or_else(|| InfraError::Unexpected {
                        cause: "Redmine import journal has no journal ID".to_string(),
                    })?,
                redmine_user_id: *author.redmine_user_id(),
                author_name: author.display_name().clone(),
                content: comment.content().to_string(),
                timestamp: *comment.timestamp(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    journals.sort_by_key(|journal| journal.journal_id);

    let mut label_ids = issue
        .label_ids()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    label_ids.sort();

    Ok(ImportedPayload {
        form_id: answer.form_id().to_string(),
        redmine_user_id: *author.redmine_user_id(),
        author_name: author.display_name().clone(),
        title,
        publication: answer.publication().to_string(),
        status: answer.status().to_string(),
        timestamp: *answer.timestamp(),
        contents,
        journals,
        label_ids,
    })
}

async fn verify_issue_in_transaction(
    txn: &mut DatabaseTransaction,
    issue: &RedmineImportedIssue,
    lock_existing: bool,
) -> Result<RedmineImportVerification, InfraError> {
    let issue_id = issue
        .answer()
        .redmine_reference()
        .ok_or_else(|| InfraError::Unexpected {
            cause: "Redmine import answer is missing its issue reference".to_string(),
        })?
        .issue_id()
        .into_inner();
    let existing_answer_id = if lock_existing {
        sqlx::query!(
            "SELECT answer_id FROM redmine_imported_answer_references WHERE redmine_issue_id = ? FOR UPDATE",
            issue_id,
        )
        .fetch_optional(&mut **txn)
        .await?
        .map(|row| row.answer_id.to_owned())
    } else {
        sqlx::query!(
            "SELECT answer_id FROM redmine_imported_answer_references WHERE redmine_issue_id = ?",
            issue_id,
        )
        .fetch_optional(&mut **txn)
        .await?
        .map(|row| row.answer_id.to_owned())
    };

    let Some(existing_answer_id) = existing_answer_id else {
        return Ok(RedmineImportVerification::ImportRequired);
    };
    let existing = load_existing_payload(txn, &existing_answer_id).await?;
    let candidate = candidate_payload(issue)?;
    if existing == candidate {
        Ok(RedmineImportVerification::AlreadyImported)
    } else {
        Err(InfraError::Unexpected {
            cause: format!("Redmine issue {issue_id} already exists with a different payload"),
        })
    }
}

async fn load_existing_payload(
    txn: &mut DatabaseTransaction,
    answer_id: &str,
) -> Result<ImportedPayload, InfraError> {
    let answer = sqlx::query!(
        r"SELECT form_id, author_type, redmine_user_id, redmine_author_name, title,
            publication, status, timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>`
        FROM answers
        WHERE id = ?",
        answer_id,
    )
    .fetch_optional(&mut **txn)
    .await?
    .ok_or_else(|| InfraError::Unexpected {
        cause: format!("Redmine reference points to missing answer {answer_id}"),
    })?;
    if answer.author_type != "IMPORTED_FROM_REDMINE" {
        return Err(InfraError::Unexpected {
            cause: format!("Redmine reference points to non-imported answer {answer_id}"),
        });
    }
    let author_name = answer
        .redmine_author_name
        .ok_or_else(|| InfraError::Unexpected {
            cause: format!("imported answer {answer_id} has no Redmine author name"),
        })?;

    let content_rows = sqlx::query!(
        "SELECT question_id, answer FROM real_answers WHERE answer_id = ? ORDER BY question_id",
        answer_id,
    )
    .fetch_all(&mut **txn)
    .await?;
    let contents = content_rows
        .into_iter()
        .map(|row| (row.question_id, row.answer))
        .collect();

    let journal_rows = sqlx::query!(
        r"SELECT redmine_journal_id, redmine_user_id, redmine_author_name, content,
            timestamp AS `timestamp!: chrono::DateTime<chrono::Utc>`
        FROM redmine_imported_comments
        WHERE answer_id = ?
        ORDER BY redmine_journal_id",
        answer_id,
    )
    .fetch_all(&mut **txn)
    .await?;
    let journals = journal_rows
        .into_iter()
        .map(|row| ImportedJournalPayload {
            journal_id: row.redmine_journal_id,
            redmine_user_id: row.redmine_user_id,
            author_name: row.redmine_author_name,
            content: row.content,
            timestamp: row.timestamp,
        })
        .collect();

    let label_rows = sqlx::query!(
        r"SELECT label_id
        FROM label_settings_for_form_answers
        WHERE answer_id = ?
        ORDER BY label_id",
        answer_id,
    )
    .fetch_all(&mut **txn)
    .await?;
    let label_ids = label_rows.into_iter().map(|row| row.label_id).collect();

    Ok(ImportedPayload {
        form_id: answer.form_id,
        redmine_user_id: answer.redmine_user_id,
        author_name,
        title: answer.title,
        publication: answer.publication,
        status: answer.status,
        timestamp: answer.timestamp,
        contents,
        journals,
        label_ids,
    })
}

async fn import_issue_in_transaction(
    txn: &mut DatabaseTransaction,
    issue: RedmineImportedIssue,
) -> Result<RedmineImportResult, InfraError> {
    if verify_issue_in_transaction(txn, &issue, true).await?
        == RedmineImportVerification::AlreadyImported
    {
        return Ok(RedmineImportResult::AlreadyImported);
    }

    let (answer, comments, label_ids) = issue.into_parts();
    let answer_id = answer.id().into_inner().to_string();
    let form_id = answer.form_id().into_inner().to_string();
    let reference = answer
        .redmine_reference()
        .ok_or_else(|| InfraError::Unexpected {
            cause: "Redmine import answer is missing its issue reference".to_string(),
        })?;
    let (author_type, redmine_user_id, redmine_author_name) = match answer.author() {
        AnswerAuthor::ImportedFromRedmine(author) => (
            "IMPORTED_FROM_REDMINE",
            *author.redmine_user_id(),
            Some(author.display_name().clone()),
        ),
        _ => {
            return Err(InfraError::Unexpected {
                cause: "Redmine import answer has a non-imported author".to_string(),
            });
        }
    };
    let title = answer
        .title()
        .clone()
        .into_inner()
        .map(|title| title.into_inner());

    sqlx::query!(
        r"SELECT id FROM form_meta_data WHERE id = ? FOR UPDATE",
        form_id,
    )
    .fetch_optional(&mut **txn)
    .await?
    .ok_or_else(|| InfraError::Unexpected {
        cause: format!("import target form does not exist: {form_id}"),
    })?;

    for content in answer.contents() {
        let question_id = content.question_id.to_string();
        let question = sqlx::query!(
            "SELECT question_id FROM form_questions WHERE form_id = ? AND question_id = ?",
            form_id,
            question_id,
        )
        .fetch_optional(&mut **txn)
        .await?;
        if question.is_none() {
            return Err(InfraError::Unexpected {
                cause: format!(
                    "answer question {question_id} does not belong to import form {form_id}"
                ),
            });
        }
    }
    for label_id in &label_ids {
        let label_id = label_id.into_inner().to_string();
        let label = sqlx::query!(
            "SELECT id FROM label_for_form_answers WHERE id = ?",
            label_id,
        )
        .fetch_optional(&mut **txn)
        .await?;
        if label.is_none() {
            return Err(InfraError::Unexpected {
                cause: format!("answer label does not exist: {label_id}"),
            });
        }
    }
    for comment in &comments {
        let journal_id = comment
            .redmine_journal_id()
            .ok_or_else(|| InfraError::Unexpected {
                cause: "Redmine import comment is missing its journal ID".to_string(),
            })?;
        let existing_journal = sqlx::query!(
            "SELECT answer_id FROM redmine_imported_comments WHERE redmine_journal_id = ?",
            journal_id,
        )
        .fetch_optional(&mut **txn)
        .await?;
        if existing_journal.is_some() {
            return Err(InfraError::Unexpected {
                cause: format!("Redmine journal {journal_id} already exists"),
            });
        }
    }

    sqlx::query!(
        r"INSERT INTO answers
            (id, form_id, author_type, redmine_user_id, redmine_author_name, title,
             publication, status, timestamp)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        answer_id,
        form_id,
        author_type,
        redmine_user_id,
        redmine_author_name,
        title,
        answer.publication().to_string(),
        answer.status().to_string(),
        answer.timestamp(),
    )
    .execute(&mut **txn)
    .await?;

    sqlx::query!(
        "INSERT INTO redmine_imported_answer_references (answer_id, redmine_issue_id) VALUES (?, ?)",
        answer_id,
        reference.issue_id().into_inner(),
    )
    .execute(&mut **txn)
    .await?;

    for content in answer.contents() {
        sqlx::query!(
            "INSERT INTO real_answers (id, answer_id, question_id, answer) VALUES (?, ?, ?, ?)",
            content.id.into_inner().to_string(),
            answer_id,
            content.question_id.into_inner().to_string(),
            content.answer,
        )
        .execute(&mut **txn)
        .await?;
    }

    for comment in comments {
        let author = comment
            .redmine_author()
            .ok_or_else(|| InfraError::Unexpected {
                cause: "Redmine import comment is missing its author".to_string(),
            })?;
        sqlx::query!(
            r"INSERT INTO redmine_imported_comments
                (comment_id, answer_id, redmine_journal_id, redmine_user_id,
                 redmine_author_name, content, timestamp)
            VALUES (?, ?, ?, ?, ?, ?, ?)",
            comment.comment_id().into_inner().to_string(),
            answer_id,
            comment
                .redmine_journal_id()
                .ok_or_else(|| InfraError::Unexpected {
                    cause: "Redmine import comment is missing its journal ID".to_string(),
                })?,
            *author.redmine_user_id(),
            author.display_name(),
            comment.content().to_string(),
            comment.timestamp(),
        )
        .execute(&mut **txn)
        .await?;
    }

    for label_id in label_ids {
        sqlx::query!(
            "INSERT INTO label_settings_for_form_answers (answer_id, label_id) VALUES (?, ?)",
            answer_id,
            label_id.into_inner().to_string(),
        )
        .execute(&mut **txn)
        .await?;
    }

    Ok(RedmineImportResult::Imported)
}
