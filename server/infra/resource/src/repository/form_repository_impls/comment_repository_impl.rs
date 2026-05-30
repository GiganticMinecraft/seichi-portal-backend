use async_trait::async_trait;
use domain::{
    form::{answer::models::AnswerEntry, comment::models::Comment},
    repository::form::comment_repository::CommentRepository,
    types::authorization_guard::{Allowed, Create, Delete, Read, Update},
};
use errors::Error;

use crate::{
    database::{
        components::{DatabaseComponents, FormCommentDatabase},
        connection::DatabaseTransaction,
    },
    repository::Repository,
};

#[async_trait]
impl<Client> CommentRepository for Repository<Client>
where
    Client: DatabaseComponents<TransactionAcrossComponents = DatabaseTransaction> + 'static,
{
    #[tracing::instrument(skip(self, comment))]
    async fn create(&self, comment: Allowed<Comment, Create>) -> Result<(), Error> {
        let comment = comment.into_inner();

        self.client
            .form_comment()
            .upsert_comment(*comment.answer_id(), &comment)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self, answer))]
    async fn find_by_answer(
        &self,
        answer: &Allowed<AnswerEntry, Read>,
    ) -> Result<Vec<Allowed<Comment, Read>>, Error> {
        self.client
            .form_comment()
            .get_comments(*answer.value().id())
            .await?
            .into_iter()
            .map(|record| {
                let comment = TryInto::<Comment>::try_into(record)?;
                answer.authorize_comment(comment).map_err(Error::from)
            })
            .collect()
    }

    #[tracing::instrument(skip(self, comment))]
    async fn update(&self, comment: Allowed<Comment, Update>) -> Result<(), Error> {
        let comment = comment.into_inner();

        self.client
            .form_comment()
            .upsert_comment(*comment.answer_id(), &comment)
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self, comment))]
    async fn delete(&self, comment: Allowed<Comment, Delete>) -> Result<(), Error> {
        let comment = comment.into_inner();

        self.client
            .form_comment()
            .delete_comment(*comment.comment_id())
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn size(&self) -> Result<u32, Error> {
        self.client.form_comment().size().await.map_err(Into::into)
    }
}
