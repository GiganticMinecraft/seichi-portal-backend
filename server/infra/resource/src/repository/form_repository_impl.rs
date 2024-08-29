use async_trait::async_trait;
use domain::{
    form::models::{
        AnswerId, Comment, CommentId, Form, FormDescription, FormId, FormQuestionUpdateSchema,
        FormTitle, FormUpdateTargets, Label, LabelId, LabelSchema, OffsetAndLimit, PostedAnswers,
        PostedAnswersSchema, PostedAnswersUpdateSchema, Question, SimpleForm,
    },
    repository::form_repository::FormRepository,
    user::models::User,
};
use errors::{infra::InfraError::AnswerNotFount, Error};
use futures::{stream, stream::StreamExt};
use outgoing::form_outgoing;
use types::Resolver;

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

    async fn public_list(
        &self,
        offset_and_limit: OffsetAndLimit,
    ) -> Result<Vec<SimpleForm>, Error> {
        let forms = self.client.form().public_list(offset_and_limit).await?;
        forms
            .into_iter()
            .map(|form| form.try_into())
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
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
    async fn post_answer(&self, user: &User, answers: &PostedAnswersSchema) -> Result<(), Error> {
        let form = self.get(answers.form_id).await?;
        form_outgoing::post_answer(&form, user, answers).await?;

        self.client
            .form()
            .post_answer(user, answers)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn get_answers(&self, answer_id: AnswerId) -> Result<Option<PostedAnswers>, Error> {
        self.client
            .form()
            .get_answers(answer_id)
            .await?
            .map(|posted_answers_dto| Ok(posted_answers_dto.try_into()?))
            .transpose()
    }

    async fn get_answers_by_form_id(&self, form_id: FormId) -> Result<Vec<PostedAnswers>, Error> {
        self.client
            .form()
            .get_answers_by_form_id(form_id)
            .await
            .map(|answers| {
                answers
                    .into_iter()
                    .map(|posted_answers_dto| posted_answers_dto.try_into())
                    .collect::<Result<Vec<PostedAnswers>, _>>()
            })?
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

    #[tracing::instrument(skip(self))]
    async fn update_answer_meta(
        &self,
        answer_id: AnswerId,
        posted_answers_update_schema: &PostedAnswersUpdateSchema,
    ) -> Result<(), Error> {
        self.client
            .form()
            .update_answer_meta(answer_id, posted_answers_update_schema)
            .await
            .map_err(Into::into)
    }

    async fn create_questions(&self, questions: &FormQuestionUpdateSchema) -> Result<(), Error> {
        self.client
            .form()
            .create_questions(questions)
            .await
            .map_err(Into::into)
    }

    async fn put_questions(&self, questions: &FormQuestionUpdateSchema) -> Result<(), Error> {
        self.client
            .form()
            .put_questions(questions)
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

    async fn post_comment(&self, answer_id: AnswerId, comment: &Comment) -> Result<(), Error> {
        let posted_answers = answer_id.resolve(self).await?.ok_or(AnswerNotFount {
            id: answer_id.into_inner(),
        })?;
        let form = self.get(posted_answers.form_id).await?;

        form_outgoing::post_comment(&form, comment, &posted_answers).await?;

        self.client
            .form()
            .post_comment(answer_id, comment)
            .await
            .map_err(Into::into)
    }

    async fn delete_comment(&self, comment_id: CommentId) -> Result<(), Error> {
        self.client
            .form()
            .delete_comment(comment_id)
            .await
            .map_err(Into::into)
    }

    async fn create_label_for_answers(&self, label: &LabelSchema) -> Result<(), Error> {
        self.client
            .form()
            .create_label_for_answers(label)
            .await
            .map_err(Into::into)
    }

    async fn get_labels_for_answers(&self) -> Result<Vec<Label>, Error> {
        stream::iter(self.client.form().get_labels_for_answers().await?)
            .then(|label_dto| async { Ok(label_dto.try_into()?) })
            .collect::<Vec<Result<Label, _>>>()
            .await
            .into_iter()
            .collect::<Result<Vec<Label>, _>>()
    }

    async fn delete_label_for_answers(&self, label_id: LabelId) -> Result<(), Error> {
        self.client
            .form()
            .delete_label_for_answers(label_id)
            .await
            .map_err(Into::into)
    }

    async fn edit_label_for_answers(&self, label: &Label) -> Result<(), Error> {
        self.client
            .form()
            .edit_label_for_answers(label)
            .await
            .map_err(Into::into)
    }

    async fn replace_answer_labels(
        &self,
        answer_id: AnswerId,
        label_ids: Vec<LabelId>,
    ) -> Result<(), Error> {
        self.client
            .form()
            .replace_answer_labels(answer_id, label_ids)
            .await
            .map_err(Into::into)
    }

    async fn create_label_for_forms(&self, label: &LabelSchema) -> Result<(), Error> {
        self.client
            .form()
            .create_label_for_forms(label)
            .await
            .map_err(Into::into)
    }

    async fn get_labels_for_forms(&self) -> Result<Vec<Label>, Error> {
        stream::iter(self.client.form().get_labels_for_forms().await?)
            .then(|label_dto| async { Ok(label_dto.try_into()?) })
            .collect::<Vec<Result<Label, _>>>()
            .await
            .into_iter()
            .collect::<Result<Vec<Label>, _>>()
    }

    async fn delete_label_for_forms(&self, label_id: LabelId) -> Result<(), Error> {
        self.client
            .form()
            .delete_label_for_forms(label_id)
            .await
            .map_err(Into::into)
    }

    async fn edit_label_for_forms(&self, label: &Label) -> Result<(), Error> {
        self.client
            .form()
            .edit_label_for_forms(label)
            .await
            .map_err(Into::into)
    }

    async fn replace_form_labels(
        &self,
        form_id: FormId,
        label_ids: Vec<LabelId>,
    ) -> Result<(), Error> {
        self.client
            .form()
            .replace_form_labels(form_id, label_ids)
            .await
            .map_err(Into::into)
    }
}
