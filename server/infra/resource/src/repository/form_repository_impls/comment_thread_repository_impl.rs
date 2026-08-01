use async_trait::async_trait;
use domain::{
    account::models::UserSnapshot,
    form::{
        answer::AnswerEntry,
        comment::{
            Comment, CommentContent, CommentHistoryAction, CommentHistoryEntry,
            CommentHistoryPagePosition, DeletedComment,
        },
        comment_thread::CommentThread,
        models::ActiveForm,
    },
    pagination::{Page, PageRequest},
    repository::form::comment_thread_repository::CommentThreadRepository,
    types::authorization_guard::{Allowed, Create, Read, Update},
};
use errors::{Error, infra::InfraError};
use std::str::FromStr;
use uuid::Uuid;

use crate::{
    database::components::{DatabaseComponents, FormCommentDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client> CommentThreadRepository for Repository<Client>
where
    Client: DatabaseComponents + 'static,
{
    #[tracing::instrument(skip_all)]
    async fn get_for_answer(
        &self,
        form: &Allowed<ActiveForm, Read>,
        answer: AnswerEntry,
    ) -> Result<Allowed<CommentThread, Read>, Error> {
        form.comment_thread(answer).map_err(Error::from)
    }

    #[tracing::instrument(skip_all)]
    async fn get_with_comments_for_answer(
        &self,
        form: &Allowed<ActiveForm, Read>,
        answer: AnswerEntry,
    ) -> Result<Allowed<CommentThread, Read>, Error> {
        let comments = self
            .client
            .form_comment()
            .get_comments(*answer.id())
            .await?
            .into_iter()
            .map(TryInto::try_into)
            .collect::<Result<Vec<_>, _>>()?;
        form.comment_thread_with_comments(answer, comments)
            .map_err(Error::from)
    }

    #[tracing::instrument(skip_all)]
    async fn create(
        &self,
        form: &Allowed<ActiveForm, Read>,
        comment: Allowed<Comment, Create>,
    ) -> Result<(), Error> {
        self.client
            .form_comment()
            .create_comment_authorizing_in_transaction(form, comment)
            .await
    }

    #[tracing::instrument(skip_all)]
    async fn update(
        &self,
        form: &Allowed<ActiveForm, Read>,
        comment: Allowed<Comment, Update>,
        updated_at: chrono::DateTime<chrono::Utc>,
    ) -> Result<(), Error> {
        self.client
            .form_comment()
            .update_comment_authorizing_in_transaction(form, comment, updated_at)
            .await
    }

    #[tracing::instrument(skip_all)]
    async fn delete(
        &self,
        form: &Allowed<ActiveForm, Read>,
        comment: Allowed<DeletedComment, Create>,
    ) -> Result<(), Error> {
        self.client
            .form_comment()
            .delete_comment_authorizing_in_transaction(form, comment)
            .await
    }

    async fn history(
        &self,
        comment_thread: &Allowed<CommentThread, Read>,
        request: PageRequest<CommentHistoryPagePosition>,
    ) -> Result<Page<Allowed<CommentHistoryEntry, Read>, CommentHistoryPagePosition>, Error> {
        let page = self
            .client
            .form_comment()
            .get_history(
                *comment_thread.answer_id(),
                request,
                comment_thread.can_read_deleted_comment_history(),
            )
            .await?;
        let (records, next) = page.into_parts();
        let items = records
            .into_iter()
            .map(|record| {
                let action = comment_history_action(record.action.as_str())?;
                let history_entry = unsafe {
                    CommentHistoryEntry::from_raw_parts(
                        Uuid::parse_str(&record.id)
                            .map_err(InfraError::from)?
                            .into(),
                        Uuid::parse_str(&record.answer_id)
                            .map_err(InfraError::from)?
                            .into(),
                        Uuid::parse_str(&record.comment_id)
                            .map_err(InfraError::from)?
                            .into(),
                        UserSnapshot::new(
                            Uuid::parse_str(&record.original_author_id)
                                .map_err(InfraError::from)?
                                .into(),
                            record.original_author_name,
                            domain::account::models::Role::from_str(&record.original_author_role)
                                .map_err(InfraError::from)?,
                        ),
                        record.original_timestamp,
                        action,
                        CommentContent::new(record.content.try_into()?),
                        UserSnapshot::new(
                            Uuid::parse_str(&record.operated_by_id)
                                .map_err(InfraError::from)?
                                .into(),
                            record.operated_by_name,
                            domain::account::models::Role::from_str(&record.operated_by_role)
                                .map_err(InfraError::from)?,
                        ),
                        record.operated_at,
                    )
                };
                comment_thread
                    .authorize_comment_history_entry(history_entry)
                    .map_err(Error::from)
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(Page::new(items, next))
    }

    async fn size(&self) -> Result<u32, Error> {
        self.client.form_comment().size().await.map_err(Into::into)
    }
}

fn comment_history_action(action: &str) -> Result<CommentHistoryAction, InfraError> {
    match action {
        "CREATE" => Ok(CommentHistoryAction::Create),
        "UPDATE" => Ok(CommentHistoryAction::Update),
        "DELETE" => Ok(CommentHistoryAction::Delete),
        action => Err(InfraError::Unexpected {
            cause: format!("invalid comment history payload for action: {action}"),
        }),
    }
}
