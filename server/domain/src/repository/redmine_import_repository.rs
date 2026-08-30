use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::redmine_import::{
        RedmineImportAnswerRelationsResult, RedmineImportResult, RedmineImportTarget,
        RedmineImportVerification, RedmineImportedIssue, RedmineIssueRelationBatch,
    },
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Read},
};

/// Redmine 移行専用の保存境界です。
///
/// 通常の回答・コメント Repository とは分離し、issue 単位の transaction、既存 payload
/// の照合、Imported データの一括保存を同じ入口で扱います。
#[automock]
#[async_trait]
pub trait RedmineImportRepository: Send + Sync + 'static {
    async fn find_target(
        &self,
        form_id: crate::form::models::FormId,
        form_title: &str,
        label_names: &[String],
    ) -> Result<Option<AuthorizationGuard<RedmineImportTarget, Read>>, Error>;

    async fn verify_issue(
        &self,
        issue: &Allowed<RedmineImportedIssue, Read>,
    ) -> Result<RedmineImportVerification, Error>;

    async fn import_issue(
        &self,
        issue: Allowed<RedmineImportedIssue, Create>,
    ) -> Result<RedmineImportResult, Error>;

    async fn import_answer_relations(
        &self,
        relations: Allowed<RedmineIssueRelationBatch, Create>,
    ) -> Result<RedmineImportAnswerRelationsResult, Error>;
}
