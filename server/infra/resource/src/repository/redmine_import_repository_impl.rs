use async_trait::async_trait;
use domain::{
    form::redmine_import::{
        RedmineImportAnswerRelationsResult, RedmineImportResult, RedmineImportTarget,
        RedmineImportVerification, RedmineImportedIssue, RedmineIssueRelationBatch,
    },
    repository::redmine_import_repository::RedmineImportRepository,
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Read},
};
use errors::Error;

use crate::{
    database::{connection::RedmineImportConnectionPool, redmine_import::RedmineImportDatabase},
    repository::Repository,
};

#[async_trait]
impl RedmineImportRepository for Repository<RedmineImportConnectionPool> {
    async fn find_target(
        &self,
        form_id: domain::form::models::FormId,
        form_title: &str,
        label_names: &[String],
    ) -> Result<Option<AuthorizationGuard<RedmineImportTarget, Read>>, Error> {
        self.client
            .find_target(form_id, form_title, label_names)
            .await
            .map(|target| target.map(AuthorizationGuard::from))
            .map_err(Into::into)
    }

    async fn verify_issue(
        &self,
        issue: &Allowed<RedmineImportedIssue, Read>,
    ) -> Result<RedmineImportVerification, Error> {
        self.client
            .verify_issue(issue.value())
            .await
            .map_err(Into::into)
    }

    async fn import_issue(
        &self,
        issue: Allowed<RedmineImportedIssue, Create>,
    ) -> Result<RedmineImportResult, Error> {
        self.client
            .import_issue(issue.into_inner())
            .await
            .map_err(Into::into)
    }

    async fn import_answer_relations(
        &self,
        relations: Allowed<RedmineIssueRelationBatch, Create>,
    ) -> Result<RedmineImportAnswerRelationsResult, Error> {
        self.client
            .import_answer_relations(relations.into_inner())
            .await
            .map_err(Into::into)
    }
}
