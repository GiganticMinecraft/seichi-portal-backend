use std::collections::HashSet;

use errors::domain::DomainError;

use crate::{
    auth::Actor,
    form::{
        answer::{AnswerAuthor, AnswerEntry, AnswerLabelId, RedmineIssueId},
        comment::{Comment, CommentSource},
        models::FormId,
        question::QuestionSet,
    },
    types::authorization_guard::{AuthorizationGuardDefinitions, AuthorizationRole, SelfGuarded},
};

/// Redmine の tracker を Portal のフォームへ割り当てるために必要な既存データです。
///
/// この値は Repository がデータベースから読み込み、Usecase が `Actor::System` の
/// Read proof を得てから移行対象の回答を組み立てるために使います。フォームや label を
/// この型から新しく作ることはありません。
#[derive(Clone, Debug, PartialEq)]
pub struct RedmineImportTarget {
    form_id: FormId,
    questions: QuestionSet,
    label_ids: Vec<AnswerLabelId>,
}

impl RedmineImportTarget {
    pub fn try_new(
        form_id: FormId,
        questions: QuestionSet,
        label_ids: Vec<AnswerLabelId>,
    ) -> Result<Self, DomainError> {
        let mut unique_label_ids = HashSet::with_capacity(label_ids.len());
        if label_ids
            .iter()
            .any(|label_id| !unique_label_ids.insert(*label_id))
        {
            return Err(DomainError::InvalidEntity {
                message: "Redmine import target contains duplicate answer labels".to_string(),
            });
        }

        Ok(Self {
            form_id,
            questions,
            label_ids,
        })
    }

    pub fn form_id(&self) -> &FormId {
        &self.form_id
    }

    pub fn questions(&self) -> &QuestionSet {
        &self.questions
    }

    pub fn label_ids(&self) -> &[AnswerLabelId] {
        &self.label_ids
    }
}

impl AuthorizationRole for RedmineImportTarget {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for RedmineImportTarget {
    fn can_create(&self, _actor: &Actor) -> bool {
        false
    }

    fn can_read(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::System)
    }

    fn can_update(&self, _actor: &Actor) -> bool {
        false
    }

    fn can_delete(&self, _actor: &Actor) -> bool {
        false
    }
}

/// Redmine issue と、その回答・journal・label を一つの保存単位として表します。
///
/// 作成途中の `AnswerEntry`、Portal 用のコメント、重複した journal/label を Repository
/// へ渡さないため、外部データから組み立てる際は必ず [`Self::try_new`] を通します。
#[derive(Clone, Debug, PartialEq)]
pub struct RedmineImportedIssue {
    answer: AnswerEntry,
    comments: Vec<Comment>,
    label_ids: Vec<AnswerLabelId>,
}

impl RedmineImportedIssue {
    pub fn try_new(
        answer: AnswerEntry,
        comments: Vec<Comment>,
        label_ids: Vec<AnswerLabelId>,
    ) -> Result<Self, DomainError> {
        let Some(reference) = answer.redmine_reference() else {
            return Err(DomainError::InvalidEntity {
                message: "Redmine imported answer must have an issue reference".to_string(),
            });
        };
        let AnswerAuthor::ImportedFromRedmine(answer_author) = answer.author() else {
            return Err(DomainError::InvalidEntity {
                message: "Redmine imported answer author and reference are inconsistent"
                    .to_string(),
            });
        };
        if reference.answer_id() != answer.id() {
            return Err(DomainError::InvalidEntity {
                message: "Redmine imported answer author and reference are inconsistent"
                    .to_string(),
            });
        }
        answer_author.validate()?;
        if reference.issue_id().into_inner() <= 0 {
            return Err(DomainError::InvalidEntity {
                message: "Redmine issue ID must be positive".to_string(),
            });
        }

        let mut comment_ids = HashSet::with_capacity(comments.len());
        let mut journal_ids = HashSet::with_capacity(comments.len());
        for comment in &comments {
            if comment.answer_id() != answer.id()
                || !matches!(comment.source(), CommentSource::ImportedFromRedmine { .. })
            {
                return Err(DomainError::InvalidEntity {
                    message: "Redmine imported journal must belong to the imported answer"
                        .to_string(),
                });
            }

            let Some(journal_id) = comment.redmine_journal_id() else {
                return Err(DomainError::InvalidEntity {
                    message: "Redmine imported journal is missing its journal ID".to_string(),
                });
            };
            if journal_id <= 0 {
                return Err(DomainError::InvalidEntity {
                    message: "Redmine journal ID must be positive".to_string(),
                });
            }
            let Some(comment_author) = comment.redmine_author() else {
                return Err(DomainError::InvalidEntity {
                    message: "Redmine imported journal is missing its author".to_string(),
                });
            };
            comment_author.validate()?;
            if !comment_ids.insert(*comment.comment_id()) || !journal_ids.insert(journal_id) {
                return Err(DomainError::InvalidEntity {
                    message: "Redmine imported journals must have unique IDs".to_string(),
                });
            }
        }

        let mut unique_label_ids = HashSet::with_capacity(label_ids.len());
        if label_ids
            .iter()
            .any(|label_id| !unique_label_ids.insert(*label_id))
        {
            return Err(DomainError::InvalidEntity {
                message: "Redmine imported answer contains duplicate labels".to_string(),
            });
        }

        Ok(Self {
            answer,
            comments,
            label_ids,
        })
    }

    pub fn answer(&self) -> &AnswerEntry {
        &self.answer
    }

    pub fn comments(&self) -> &[Comment] {
        &self.comments
    }

    pub fn label_ids(&self) -> &[AnswerLabelId] {
        &self.label_ids
    }

    pub fn into_parts(self) -> (AnswerEntry, Vec<Comment>, Vec<AnswerLabelId>) {
        (self.answer, self.comments, self.label_ids)
    }
}

impl AuthorizationRole for RedmineImportedIssue {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for RedmineImportedIssue {
    fn can_create(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::System)
    }

    fn can_read(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::System)
    }

    fn can_update(&self, _actor: &Actor) -> bool {
        false
    }

    fn can_delete(&self, _actor: &Actor) -> bool {
        false
    }
}

/// Redmine の二つの issue を結ぶ、対称な関連です。
///
/// Portal では Redmine の relation type を保持しないため、関連の向きは持ちません。
/// issue ID の小さい方を先に置くことで、同じ関連をどちらの向きから渡しても同じ値に
/// なります。
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RedmineIssueRelation {
    first: RedmineIssueId,
    second: RedmineIssueId,
}

impl RedmineIssueRelation {
    pub fn new(first: RedmineIssueId, second: RedmineIssueId) -> Result<Self, DomainError> {
        if first.into_inner() <= 0 || second.into_inner() <= 0 {
            return Err(DomainError::InvalidEntity {
                message: "Redmine issue ID must be positive".to_string(),
            });
        }
        if first == second {
            return Err(DomainError::InvalidEntity {
                message: "a Redmine issue cannot be related to itself".to_string(),
            });
        }

        let (first, second) = if first < second {
            (first, second)
        } else {
            (second, first)
        };

        Ok(Self { first, second })
    }

    pub fn first(self) -> RedmineIssueId {
        self.first
    }

    pub fn second(self) -> RedmineIssueId {
        self.second
    }
}

/// Redmine issue 関連を一つの import 操作として保存する batch です。
///
/// 関連は [`RedmineIssueRelation::new`] で正規化され、同じ正規化済みの関連を batch に
/// 二度含めることはできません。空の batch は有効で、保存時には何もしません。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RedmineIssueRelationBatch {
    relations: Vec<RedmineIssueRelation>,
}

impl RedmineIssueRelationBatch {
    pub fn new(relations: Vec<RedmineIssueRelation>) -> Result<Self, DomainError> {
        let mut unique_relations = HashSet::with_capacity(relations.len());
        if relations
            .iter()
            .any(|relation| !unique_relations.insert(*relation))
        {
            return Err(DomainError::InvalidEntity {
                message: "Redmine issue relation batch contains duplicate relations".to_string(),
            });
        }

        Ok(Self { relations })
    }

    pub fn relations(&self) -> &[RedmineIssueRelation] {
        &self.relations
    }
}

impl AuthorizationRole for RedmineIssueRelationBatch {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for RedmineIssueRelationBatch {
    fn can_create(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::System)
    }

    fn can_read(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::System)
    }

    fn can_update(&self, _actor: &Actor) -> bool {
        false
    }

    fn can_delete(&self, _actor: &Actor) -> bool {
        false
    }
}

/// Redmine issue の保存前後で Repository が返す結果です。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedmineImportResult {
    Imported,
    AlreadyImported,
}

/// 既存データを変更せずに payload を照合した結果です。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedmineImportVerification {
    ImportRequired,
    AlreadyImported,
}

/// Redmine issue 関連の保存結果です。
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RedmineImportAnswerRelationsResult {
    inserted: usize,
    already_exists: usize,
}

impl RedmineImportAnswerRelationsResult {
    pub fn new(inserted: usize, already_exists: usize) -> Self {
        Self {
            inserted,
            already_exists,
        }
    }

    pub fn inserted(self) -> usize {
        self.inserted
    }

    pub fn already_exists(self) -> usize {
        self.already_exists
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        account::models::{AccountUser, Role, UserId},
        types::authorization_guard::{AuthorizationGuard, Create},
    };
    use uuid::Uuid;

    fn issue_id(value: i64) -> RedmineIssueId {
        RedmineIssueId::try_new(value).unwrap()
    }

    #[test]
    fn relation_normalizes_issue_ids_and_rejects_invalid_endpoints() {
        let first = issue_id(10);
        let second = issue_id(20);

        let relation = RedmineIssueRelation::new(second, first).unwrap();

        assert_eq!(relation.first(), first);
        assert_eq!(relation.second(), second);
        assert!(RedmineIssueRelation::new(first, first).is_err());
        assert!(RedmineIssueRelation::new(RedmineIssueId::new(0), second).is_err());
    }

    #[test]
    fn batch_rejects_duplicate_normalized_relations_and_accepts_empty_batch() {
        let first = issue_id(10);
        let second = issue_id(20);
        let forward = RedmineIssueRelation::new(first, second).unwrap();
        let reverse = RedmineIssueRelation::new(second, first).unwrap();

        assert_eq!(forward, reverse);
        assert!(RedmineIssueRelationBatch::new(vec![forward, reverse]).is_err());
        assert_eq!(
            RedmineIssueRelationBatch::new(Vec::new())
                .unwrap()
                .relations(),
            &[]
        );
    }

    #[test]
    fn only_system_can_create_a_relation_batch() {
        let relation = RedmineIssueRelation::new(issue_id(10), issue_id(20)).unwrap();
        let system_batch = RedmineIssueRelationBatch::new(vec![relation]).unwrap();
        assert!(
            AuthorizationGuard::<_, Create>::from(system_batch)
                .try_create(Actor::System)
                .is_ok()
        );

        let standard_user = AccountUser::new(
            "standard".to_string(),
            UserId::from(Uuid::from_u128(1)),
            Role::StandardUser,
        );
        let standard_batch = RedmineIssueRelationBatch::new(vec![relation]).unwrap();
        assert!(
            AuthorizationGuard::<_, Create>::from(standard_batch)
                .try_create(Actor::from(standard_user))
                .is_err()
        );
    }
}
