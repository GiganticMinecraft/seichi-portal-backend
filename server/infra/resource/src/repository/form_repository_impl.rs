use async_trait::async_trait;
use domain::{
    form::models::{
        AnswerId, Comment, Form, FormDescription, FormId, FormQuestionUpdateSchema, FormTitle,
        FormUpdateTargets, OffsetAndLimit, PostedAnswers, Question, SimpleForm,
    },
    repository::form_repository::FormRepository,
    user::models::User,
};
use errors::Error;
use futures::{stream, stream::StreamExt};
use outgoing::form_outgoing;

use crate::{
    database::components::{DatabaseComponents, FormDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> FormRepository for Repository<Client> {
    #[tracing::instrument(skip(self))]
    async fn create(
        &self,
        title: FormTitle,
        description: FormDescription,
        user: User,
    ) -> Result<FormId, Error> {
        let form_id = self.client.form().create(title, description, user).await?;
        let form = self.client.form().get(form_id).await?;

        form_outgoing::create(form.try_into()?).await?;

        Ok(form_id)
    }

    #[tracing::instrument(skip(self))]
    async fn list(&self, offset_and_limit: OffsetAndLimit) -> Result<Vec<SimpleForm>, Error> {
        let forms = self.client.form().list(offset_and_limit).await?;
        forms
            .into_iter()
            .map(|form| form.try_into())
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn get(&self, id: FormId) -> Result<Form, Error> {
        let form = self.client.form().get(id).await?;
        form.try_into().map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn delete(&self, id: FormId) -> Result<(), Error> {
        let form = self.client.form().get(id).await?;

        form_outgoing::delete(form.try_into()?).await?;

        self.client.form().delete(id).await.map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn update(
        &self,
        form_id: FormId,
        form_update_targets: FormUpdateTargets,
    ) -> Result<(), Error> {
        self.client
            .form()
            .update(form_id, form_update_targets)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn post_answer(&self, answers: PostedAnswers) -> Result<(), Error> {
        let form = self.get(answers.form_id).await?;
        form_outgoing::post_answer(&form, &answers).await?;

        self.client
            .form()
            .post_answer(answers)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn get_all_answers(&self) -> Result<Vec<PostedAnswers>, Error> {
        stream::iter(self.client.form().get_all_answers().await?)
            .then(|posted_answers_dto| async { Ok(posted_answers_dto.try_into()?) })
            .collect::<Vec<Result<PostedAnswers, _>>>()
            .await
            .into_iter()
            .collect::<Result<Vec<PostedAnswers>, _>>()
    }

    async fn create_questions(&self, questions: FormQuestionUpdateSchema) -> Result<(), Error> {
        self.client
            .form()
            .create_questions(questions)
            .await
            .map_err(Into::into)
    }

    async fn get_questions(&self, form_id: FormId) -> Result<Vec<Question>, Error> {
        self.client
            .form()
            .get_questions(form_id)
            .await
            .map(|questions_dto| {
                questions_dto
                    .into_iter()
                    .map(|question_dto| question_dto.try_into())
                    .collect::<Result<Vec<Question>, _>>()
            })?
            .map_err(Into::into)
    }

    async fn has_permission(&self, answer_id: AnswerId, user: &User) -> Result<bool, Error> {
        self.client
            .form()
            .has_permission(answer_id, user)
            .await
            .map_err(Into::into)
    }

    async fn post_comment(&self, comment: &Comment) -> Result<(), Error> {
        self.client
            .form()
            .post_comment(comment)
            .await
            .map_err(Into::into)
    }
}
