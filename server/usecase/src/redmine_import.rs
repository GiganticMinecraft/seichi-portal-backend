use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use domain::{
    auth::Actor,
    form::{
        answer::{AnswerEntry, AnswerPublication, AnswerStatus, AnswerTitle},
        comment::{Comment, CommentContent},
        redmine_import::{
            RedmineImportAnswerRelationsResult, RedmineImportResult, RedmineImportTarget,
            RedmineImportVerification, RedmineImportedIssue, RedmineIssueRelationBatch,
        },
    },
    repository::redmine_import_repository::RedmineImportRepository,
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Read},
};
use errors::{Error, domain::DomainError};
use types::non_empty_string::NonEmptyString;

/// Redmine issue を Domain へ渡す前の、API 依存を持たない入力値です。
#[derive(Debug)]
pub struct RedmineIssueInput {
    issue_id: domain::form::answer::RedmineIssueId,
    title: String,
    question_values: BTreeMap<String, String>,
    author: domain::form::answer::RedmineUserSnapshot,
    created_at: DateTime<Utc>,
    status: AnswerStatus,
    journals: Vec<RedmineJournalInput>,
}

impl RedmineIssueInput {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        issue_id: domain::form::answer::RedmineIssueId,
        title: String,
        question_values: BTreeMap<String, String>,
        author: domain::form::answer::RedmineUserSnapshot,
        created_at: DateTime<Utc>,
        status: AnswerStatus,
        journals: Vec<RedmineJournalInput>,
    ) -> Result<Self, Error> {
        if title.trim().is_empty() {
            return Err(DomainError::InvalidEntity {
                message: "Redmine issue subject must not be empty".to_string(),
            }
            .into());
        }

        Ok(Self {
            issue_id,
            title,
            question_values,
            author,
            created_at,
            status,
            journals,
        })
    }
}

/// Redmine journal を Domain の Imported comment へ変換するための入力値です。
#[derive(Debug)]
pub struct RedmineJournalInput {
    journal_id: i64,
    author: domain::form::answer::RedmineUserSnapshot,
    notes: String,
    created_at: DateTime<Utc>,
}

impl RedmineJournalInput {
    pub fn new(
        journal_id: i64,
        author: domain::form::answer::RedmineUserSnapshot,
        notes: String,
        created_at: DateTime<Utc>,
    ) -> Result<Self, Error> {
        if journal_id <= 0 {
            return Err(DomainError::InvalidEntity {
                message: "Redmine journal ID must be positive".to_string(),
            }
            .into());
        }
        if notes.trim().is_empty() {
            return Err(DomainError::InvalidEntity {
                message: format!("Redmine journal {journal_id} has empty notes"),
            }
            .into());
        }

        Ok(Self {
            journal_id,
            author,
            notes,
            created_at,
        })
    }
}

/// Imported 回答専用の Usecase です。
///
/// 外部 API の取得や DB の SQL は持たず、既存フォームへの厳密な割り当てと Domain の
/// 一括保存単位を組み立てます。保存や照合は `RedmineImportRepository` に委譲します。
pub struct RedmineImportUseCase<'a, Repository> {
    repository: &'a Repository,
}

impl<'a, Repository> RedmineImportUseCase<'a, Repository> {
    pub fn new(repository: &'a Repository) -> Self {
        Self { repository }
    }
}

impl<Repository> RedmineImportUseCase<'_, Repository>
where
    Repository: RedmineImportRepository,
{
    pub async fn find_target(
        &self,
        form_id: domain::form::models::FormId,
        form_title: &str,
        label_names: &[String],
    ) -> Result<Option<Allowed<RedmineImportTarget, Read>>, Error> {
        self.repository
            .find_target(form_id, form_title, label_names)
            .await?
            .map(|target| target.try_read(Actor::System))
            .transpose()
            .map_err(Into::into)
    }

    pub async fn verify_issue(
        &self,
        issue: &RedmineImportedIssue,
    ) -> Result<RedmineImportVerification, Error> {
        let issue = AuthorizationGuard::<_, Read>::from(issue.clone()).try_read(Actor::System)?;
        self.repository.verify_issue(&issue).await
    }

    pub async fn import_issue(
        &self,
        issue: RedmineImportedIssue,
    ) -> Result<RedmineImportResult, Error> {
        let issue = AuthorizationGuard::<_, Create>::from(issue).try_create(Actor::System)?;
        self.repository.import_issue(issue).await
    }

    pub async fn import_answer_relations(
        &self,
        relations: RedmineIssueRelationBatch,
    ) -> Result<RedmineImportAnswerRelationsResult, Error> {
        let relations =
            AuthorizationGuard::<_, Create>::from(relations).try_create(Actor::System)?;
        self.repository.import_answer_relations(relations).await
    }
}

/// tracker の mapping と Redmine の issue を一つの Imported 集約へ変換します。
pub fn prepare_issue(
    target: &Allowed<RedmineImportTarget, Read>,
    input: RedmineIssueInput,
    publication: AnswerPublication,
) -> Result<RedmineImportedIssue, Error> {
    let RedmineIssueInput {
        issue_id,
        title: input_title,
        question_values,
        author,
        created_at,
        status,
        journals,
    } = input;
    let mut contents = question_values
        .into_iter()
        .map(|(template_key, answer)| {
            let question = target
                .questions()
                .iter()
                .find(|question| question.template_key().as_str() == template_key)
                .ok_or_else(|| DomainError::InvalidEntity {
                    message: format!("question mapping does not match form: {template_key}"),
                })?;
            Ok::<_, Error>(domain::form::answer::FormAnswerContent {
                id: domain::form::answer::FormAnswerContentId::new(),
                question_id: question.id(),
                answer,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    contents.sort_by_key(|content| {
        target
            .questions()
            .iter()
            .find(|question| question.id() == content.question_id)
            .map_or(u16::MAX, |question| question.position())
    });
    let title = AnswerTitle::new(Some(NonEmptyString::try_new(input_title)?));
    let contents = domain::form::answer::PostedAnswerContents::try_new(
        target.questions().as_slice(),
        contents,
    )?;
    let answer = AnswerEntry::import_from_redmine(
        *target.form_id(),
        issue_id,
        author,
        created_at,
        title,
        publication,
        status,
        contents,
    )
    .map_err(Error::from)?;

    let comments = journals
        .into_iter()
        .map(|journal| {
            let content =
                CommentContent::new(NonEmptyString::try_new(journal.notes).map_err(Error::from)?);
            Comment::new_imported_from_redmine(
                *answer.id(),
                journal.journal_id,
                journal.author,
                content,
                journal.created_at,
            )
            .map_err(Error::from)
        })
        .collect::<Result<Vec<_>, _>>()?;

    RedmineImportedIssue::try_new(answer, comments, target.label_ids().to_vec()).map_err(Into::into)
}

/// issue の実値がなくても、設定ファイルが指定した固定値をフォームの質問へ
/// 保存できるか検証します。必須質問を含むフォーム全体ではなく、固定値の質問だけを
/// 対象にすることで、subject/description のような動的値を仮定せずに choice を検証できます。
pub fn validate_question_value(
    target: &Allowed<RedmineImportTarget, Read>,
    template_key: &str,
    answer: String,
) -> Result<(), Error> {
    let question = target
        .questions()
        .iter()
        .find(|question| question.template_key().as_str() == template_key)
        .ok_or_else(|| DomainError::InvalidEntity {
            message: format!("question mapping does not match form: {template_key}"),
        })?;
    let content = domain::form::answer::FormAnswerContent {
        id: domain::form::answer::FormAnswerContentId::new(),
        question_id: question.id(),
        answer,
    };
    domain::form::answer::PostedAnswerContents::try_new(
        std::slice::from_ref(question),
        vec![content],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use domain::{
        form::{
            answer::{AnswerAuthor, RedmineIssueId, RedmineUserSnapshot},
            models::FormId,
            question::{Question, QuestionSet},
        },
        types::authorization_guard::AuthorizationGuard,
    };
    use types::non_empty_vec::NonEmptyVec;

    fn target() -> Allowed<RedmineImportTarget, Read> {
        let question = Question::new_text(
            "body".try_into().unwrap(),
            0,
            "Body".to_string().try_into().unwrap(),
            None,
            true,
        )
        .unwrap();
        let target = RedmineImportTarget::try_new(
            FormId::new(),
            QuestionSet::try_new(NonEmptyVec::try_new(vec![question]).unwrap()).unwrap(),
            Vec::new(),
        )
        .unwrap();
        AuthorizationGuard::from(target)
            .try_read(Actor::System)
            .unwrap()
    }

    #[tokio::test]
    async fn import_answer_relations_passes_a_system_create_proof_to_repository() {
        use domain::{
            form::{
                answer::RedmineIssueId,
                redmine_import::{RedmineImportAnswerRelationsResult, RedmineIssueRelation},
            },
            repository::redmine_import_repository::MockRedmineImportRepository,
        };

        let relation = RedmineIssueRelation::new(
            RedmineIssueId::try_new(10).unwrap(),
            RedmineIssueId::try_new(20).unwrap(),
        )
        .unwrap();
        let relations = RedmineIssueRelationBatch::new(vec![relation]).unwrap();
        let expected_result = RedmineImportAnswerRelationsResult::new(1, 2);
        let expected_relations = relations.clone();
        let mut repository = MockRedmineImportRepository::new();
        repository
            .expect_import_answer_relations()
            .once()
            .withf(move |actual| {
                actual.actor() == &Actor::System && actual.value() == &expected_relations
            })
            .returning(move |_| Ok(expected_result));

        let usecase = RedmineImportUseCase::new(&repository);
        assert_eq!(
            usecase.import_answer_relations(relations).await.unwrap(),
            expected_result
        );
    }

    #[test]
    fn prepare_issue_preserves_redmine_answer_and_journal_fields() {
        let issue_timestamp = Utc.with_ymd_and_hms(2024, 1, 2, 3, 4, 5).unwrap();
        let journal_timestamp = Utc.with_ymd_and_hms(2024, 1, 3, 4, 5, 6).unwrap();
        let input = RedmineIssueInput::new(
            RedmineIssueId::try_new(42).unwrap(),
            "Issue title".to_string(),
            [("body".to_string(), "Issue body".to_string())]
                .into_iter()
                .collect(),
            RedmineUserSnapshot::try_new(Some(7), "Issue author".to_string()).unwrap(),
            issue_timestamp,
            AnswerStatus::COMPLETED,
            vec![
                RedmineJournalInput::new(
                    99,
                    RedmineUserSnapshot::try_new(Some(8), "Journal author".to_string()).unwrap(),
                    "Journal notes".to_string(),
                    journal_timestamp,
                )
                .unwrap(),
            ],
        )
        .unwrap();

        let imported = prepare_issue(&target(), input, AnswerPublication::PRIVATE).unwrap();
        let answer = imported.answer();
        assert!(matches!(
            answer.author(),
            AnswerAuthor::ImportedFromRedmine(author)
                if author.redmine_user_id() == &Some(7)
                    && author.display_name() == "Issue author"
        ));
        assert_eq!(*answer.timestamp(), issue_timestamp);
        assert_eq!(*answer.status(), AnswerStatus::COMPLETED);
        assert_eq!(*answer.publication(), AnswerPublication::PRIVATE);
        assert_eq!(
            answer.title().clone().into_inner().unwrap().into_inner(),
            "Issue title"
        );
        assert_eq!(answer.contents()[0].answer, "Issue body");
        assert_eq!(
            answer.redmine_reference().unwrap().issue_id().into_inner(),
            42
        );
        assert_eq!(imported.comments().len(), 1);
        assert_eq!(imported.comments()[0].redmine_journal_id(), Some(99));
        assert_eq!(
            imported.comments()[0]
                .redmine_author()
                .unwrap()
                .display_name(),
            "Journal author"
        );
        assert_eq!(
            imported.comments()[0].content().to_string(),
            "Journal notes"
        );
        assert_eq!(*imported.comments()[0].timestamp(), journal_timestamp);
    }
}
