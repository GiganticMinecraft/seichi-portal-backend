use async_trait::async_trait;
use domain::{
    form::{
        answer::models::{AnswerId, FormAnswer, FormAnswerContent},
        models::FormId,
    },
    repository::form::answer_repository::AnswerRepository,
};
use errors::Error;
use futures::{stream, StreamExt};

use crate::{
    database::components::{DatabaseComponents, FormAnswerDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> AnswerRepository for Repository<Client> {
    #[tracing::instrument(skip(self))]
    async fn post_answer(
        &self,
        answer: &FormAnswer,
        content: Vec<FormAnswerContent>,
    ) -> Result<(), Error> {
        self.client
            .form_answer()
            .post_answer(answer, content)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn get_answers(&self, answer_id: AnswerId) -> Result<Option<FormAnswer>, Error> {
        self.client
            .form_answer()
            .get_answers(answer_id)
            .await?
            .map(|posted_answers_dto| Ok(posted_answers_dto.try_into()?))
            .transpose()
    }

    #[tracing::instrument(skip(self))]
    async fn get_answer_contents(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<FormAnswerContent>, Error> {
        self.client
            .form_answer()
            .get_answer_contents(answer_id)
            .await
            .map(|answer_contents| {
                answer_contents
                    .into_iter()
                    .map(|answer_content_dto| answer_content_dto.try_into())
                    .collect::<Result<Vec<FormAnswerContent>, _>>()
            })?
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn get_answers_by_form_id(&self, form_id: FormId) -> Result<Vec<FormAnswer>, Error> {
        self.client
            .form_answer()
            .get_answers_by_form_id(form_id)
            .await
            .map(|answers| {
                answers
                    .into_iter()
                    .map(|posted_answers_dto| posted_answers_dto.try_into())
                    .collect::<Result<Vec<FormAnswer>, _>>()
            })?
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn get_all_answers(&self) -> Result<Vec<FormAnswer>, Error> {
        stream::iter(self.client.form_answer().get_all_answers().await?)
            .then(|posted_answers_dto| async { Ok(posted_answers_dto.try_into()?) })
            .collect::<Vec<Result<FormAnswer, _>>>()
            .await
            .into_iter()
            .collect::<Result<Vec<FormAnswer>, _>>()
    }

    #[tracing::instrument(skip(self))]
    async fn update_answer_meta(
        &self,
        answer_id: AnswerId,
        title: Option<String>,
    ) -> Result<(), Error> {
        self.client
            .form_answer()
            .update_answer_meta(answer_id, title)
            .await
            .map_err(Into::into)
    }
}
