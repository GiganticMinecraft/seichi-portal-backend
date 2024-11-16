use async_trait::async_trait;
use domain::{
    form::models::{
        AnswerId, AnswerLabel, Comment, CommentId, DefaultAnswerTitle, Form, FormAnswer,
        FormAnswerContent, FormDescription, FormId, FormTitle, Label, LabelId, Message, MessageId,
        OffsetAndLimit, Question, ResponsePeriod, SimpleForm, Visibility, WebhookUrl,
    },
    repository::form_repository::FormRepository,
    types::authorization_guard::{AuthorizationGuard, Create, Delete, Read, Update},
    user::models::User,
};
use errors::{domain::DomainError, infra::InfraError::AnswerNotFount, Error};
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

        match self.client.form().get(form_id).await? {
            None => Ok(form_id),
            Some(form) => {
                form_outgoing::create(form.try_into()?).await?;

                Ok(form_id)
            }
        }
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
    async fn get(&self, id: FormId) -> Result<Option<Form>, Error> {
        let form = self.client.form().get(id).await?;
        form.map(TryInto::try_into).transpose().map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn delete(&self, id: FormId) -> Result<(), Error> {
        let form = self.client.form().get(id).await?;

        match form {
            None => Ok(()),
            Some(form) => {
                form_outgoing::delete(form.try_into()?).await?;
                self.client.form().delete(id).await.map_err(Into::into)
            }
        }
    }

    #[tracing::instrument(skip(self))]
    async fn update_title(&self, form_id: &FormId, title: &FormTitle) -> Result<(), Error> {
        self.client
            .form()
            .update_form_title(form_id, title)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn update_description(
        &self,
        form_id: &FormId,
        description: &FormDescription,
    ) -> Result<(), Error> {
        self.client
            .form()
            .update_form_description(form_id, description)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn update_response_period(
        &self,
        form_id: &FormId,
        response_period: &ResponsePeriod,
    ) -> Result<(), Error> {
        self.client
            .form()
            .update_form_response_period(form_id, response_period)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn update_webhook_url(
        &self,
        form_id: &FormId,
        webhook_url: &WebhookUrl,
    ) -> Result<(), Error> {
        self.client
            .form()
            .update_form_webhook_url(form_id, webhook_url)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn update_default_answer_title(
        &self,
        form_id: &FormId,
        default_answer_title: &DefaultAnswerTitle,
    ) -> Result<(), Error> {
        self.client
            .form()
            .update_form_default_answer_title(form_id, default_answer_title)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn update_visibility(
        &self,
        form_id: &FormId,
        visibility: &Visibility,
    ) -> Result<(), Error> {
        self.client
            .form()
            .update_form_visibility(form_id, visibility)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn update_answer_visibility(
        &self,
        form_id: &FormId,
        visibility: &Visibility,
    ) -> Result<(), Error> {
        self.client
            .form()
            .update_form_answer_visibility(form_id, visibility)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn post_answer(
        &self,
        user: &User,
        form_id: FormId,
        title: DefaultAnswerTitle,
        answers: Vec<FormAnswerContent>,
    ) -> Result<(), Error> {
        match self.get(form_id).await? {
            None => Ok(()),
            Some(form) => {
                form_outgoing::post_answer(&form, user, title, &answers).await?;
                self.client
                    .form()
                    .post_answer(user, form_id, answers)
                    .await
                    .map_err(Into::into)
            }
        }
    }

    #[tracing::instrument(skip(self))]
    async fn get_answers(&self, answer_id: AnswerId) -> Result<Option<FormAnswer>, Error> {
        self.client
            .form()
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
            .form()
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

    async fn get_answers_by_form_id(&self, form_id: FormId) -> Result<Vec<FormAnswer>, Error> {
        self.client
            .form()
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
        stream::iter(self.client.form().get_all_answers().await?)
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
            .form()
            .update_answer_meta(answer_id, title)
            .await
            .map_err(Into::into)
    }

    async fn create_questions(
        &self,
        form_id: FormId,
        questions: Vec<Question>,
    ) -> Result<(), Error> {
        self.client
            .form()
            .create_questions(form_id, questions)
            .await
            .map_err(Into::into)
    }

    async fn put_questions(&self, form_id: FormId, questions: Vec<Question>) -> Result<(), Error> {
        self.client
            .form()
            .put_questions(form_id, questions)
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

    async fn get_comments(&self, answer_id: AnswerId) -> Result<Vec<Comment>, Error> {
        self.client
            .form()
            .get_comments(answer_id)
            .await
            .map(|comments| {
                comments
                    .into_iter()
                    .map(|comment_dto| comment_dto.try_into())
                    .collect::<Result<Vec<Comment>, _>>()
            })?
            .map_err(Into::into)
    }

    async fn post_comment(&self, answer_id: AnswerId, comment: &Comment) -> Result<(), Error> {
        let posted_answers = answer_id.resolve(self).await?.ok_or(AnswerNotFount {
            id: answer_id.into_inner(),
        })?;

        match self.get(posted_answers.form_id).await? {
            None => Ok(()),
            Some(form) => {
                form_outgoing::post_comment(&form, comment, &posted_answers).await?;

                self.client
                    .form()
                    .post_comment(answer_id, comment)
                    .await
                    .map_err(Into::into)
            }
        }
    }

    async fn delete_comment(&self, comment_id: CommentId) -> Result<(), Error> {
        self.client
            .form()
            .delete_comment(comment_id)
            .await
            .map_err(Into::into)
    }

    async fn create_label_for_answers(&self, label_name: String) -> Result<(), Error> {
        self.client
            .form()
            .create_label_for_answers(label_name)
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

    #[tracing::instrument(skip(self))]
    async fn get_labels_for_answers_by_answer_id(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<AnswerLabel>, Error> {
        self.client
            .form()
            .get_labels_for_answers_by_answer_id(answer_id)
            .await
            .map(|labels| {
                labels
                    .into_iter()
                    .map(|label_dto| label_dto.try_into())
                    .collect::<Result<Vec<AnswerLabel>, _>>()
            })?
            .map_err(Into::into)
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

    async fn create_label_for_forms(&self, label_name: String) -> Result<(), Error> {
        self.client
            .form()
            .create_label_for_forms(label_name)
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

    async fn post_message(
        &self,
        actor: &User,
        message: AuthorizationGuard<Message, Create>,
    ) -> Result<(), Error> {
        Ok(message
            .try_create(actor, |message: &Message| {
                self.client.form().post_message(message)
            })?
            .await?)
    }

    async fn fetch_messages_by_answer(
        &self,
        answers: &FormAnswer,
    ) -> Result<Vec<AuthorizationGuard<Message, Read>>, Error> {
        self.client
            .form()
            .fetch_messages_by_form_answer(answers)
            .await?
            .into_iter()
            .map(|dto| {
                Ok::<Message, DomainError>(dto.try_into()?).map(|message| {
                    let message: AuthorizationGuard<Message, Create> = message.into();

                    message.into_read()
                })
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    async fn update_message_body(
        &self,
        actor: &User,
        message: AuthorizationGuard<Message, Update>,
        content: String,
    ) -> Result<(), Error> {
        message
            .try_update(actor, |message: &Message| {
                let message_id = message.id().to_owned();

                self.client.form().update_message_body(message_id, content)
            })?
            .await
            .map_err(Into::into)
    }

    async fn fetch_message(
        &self,
        message_id: &MessageId,
    ) -> Result<Option<AuthorizationGuard<Message, Read>>, Error> {
        self.client
            .form()
            .fetch_message(message_id)
            .await?
            .map(|dto| {
                Ok::<Message, DomainError>(dto.try_into()?).map(|message| {
                    let message: AuthorizationGuard<Message, Create> = message.into();

                    message.into_read()
                })
            })
            .transpose()
            .map_err(Into::into)
    }

    async fn delete_message(
        &self,
        actor: &User,
        message: AuthorizationGuard<Message, Delete>,
    ) -> Result<(), Error> {
        message
            .try_delete(actor, |message: &Message| {
                let message_id = message.id().to_owned();

                self.client.form().delete_message(message_id)
            })?
            .await
            .map_err(Into::into)
    }
}
