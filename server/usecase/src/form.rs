use chrono::Utc;
use domain::{
    form::models::{
        AnswerId, Comment, CommentId, DefaultAnswerTitle, Form, FormAnswerContent, FormDescription,
        FormId, FormTitle, Label, LabelId, Message, MessageId, OffsetAndLimit, Question,
        ResponsePeriod, SimpleForm, Visibility, Visibility::PUBLIC, WebhookUrl,
    },
    notification::models::{Notification, NotificationSource},
    repository::{
        form_repository::FormRepository, notification_repository::NotificationRepository,
    },
    types::authorization_guard::{AuthorizationGuard, Read},
    user::models::{
        Role::{Administrator, StandardUser},
        User,
    },
};
use errors::{
    usecase::UseCaseError::{
        AnswerNotFound, DoNotHavePermissionToPostFormComment, FormNotFound, MessageNotFound,
        OutOfPeriod,
    },
    Error,
};
use futures::{
    future::{join_all, OptionFuture},
    stream, try_join, StreamExt,
};
use types::Resolver;

use crate::dto::AnswerDto;

pub struct FormUseCase<'a, FormRepo: FormRepository, NotificationRepo: NotificationRepository> {
    pub form_repository: &'a FormRepo,
    pub notification_repository: &'a NotificationRepo,
}

impl<R1: FormRepository, R2: NotificationRepository> FormUseCase<'_, R1, R2> {
    pub async fn create_form(
        &self,
        title: FormTitle,
        description: FormDescription,
        user: User,
    ) -> Result<FormId, Error> {
        self.form_repository.create(title, description, user).await
    }

    pub async fn public_form_list(
        &self,
        offset_and_limit: OffsetAndLimit,
    ) -> Result<Vec<SimpleForm>, Error> {
        self.form_repository.public_list(offset_and_limit).await
    }

    pub async fn form_list(
        &self,
        offset_and_limit: OffsetAndLimit,
    ) -> Result<Vec<SimpleForm>, Error> {
        self.form_repository.list(offset_and_limit).await
    }

    pub async fn get_form(&self, form_id: FormId) -> Result<Form, Error> {
        self.form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))
    }

    pub async fn delete_form(&self, form_id: FormId) -> Result<(), Error> {
        self.form_repository.delete(form_id).await
    }

    pub async fn get_questions(&self, form_id: FormId) -> Result<Vec<Question>, Error> {
        self.form_repository.get_questions(form_id).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_form(
        &self,
        form_id: &FormId,
        title: Option<&FormTitle>,
        description: Option<&FormDescription>,
        has_response_period: Option<bool>,
        response_period: Option<&ResponsePeriod>,
        webhook: Option<&WebhookUrl>,
        default_answer_title: Option<&DefaultAnswerTitle>,
        visibility: Option<&Visibility>,
        answer_visibility: Option<&Visibility>,
    ) -> Result<(), Error> {
        let update_title: OptionFuture<_> = title
            .map(|title| self.form_repository.update_title(form_id, title))
            .into();
        let update_description: OptionFuture<_> = description
            .map(|description| {
                self.form_repository
                    .update_description(form_id, description)
            })
            .into();
        let update_response_period: OptionFuture<_> = if has_response_period.unwrap_or(false) {
            response_period
                .map(|response_period| {
                    self.form_repository
                        .update_response_period(form_id, response_period)
                })
                .into()
        } else {
            None.into()
        };
        let update_webhook: OptionFuture<_> = webhook
            .map(|webhook| self.form_repository.update_webhook_url(form_id, webhook))
            .into();
        let update_default_answer_title: OptionFuture<_> = default_answer_title
            .map(|default_answer_title| {
                self.form_repository
                    .update_default_answer_title(form_id, default_answer_title)
            })
            .into();
        let update_visibility: OptionFuture<_> = visibility
            .map(|visibility| self.form_repository.update_visibility(form_id, visibility))
            .into();
        let update_answer_visibility: OptionFuture<_> = answer_visibility
            .map(|visibility| {
                self.form_repository
                    .update_answer_visibility(form_id, visibility)
            })
            .into();

        join_all(vec![
            update_title,
            update_description,
            update_response_period,
            update_webhook,
            update_default_answer_title,
            update_visibility,
            update_answer_visibility,
        ])
        .await;

        Ok(())
    }

    pub async fn post_answers(
        &self,
        user: &User,
        form_id: FormId,
        title: DefaultAnswerTitle,
        answers: Vec<FormAnswerContent>,
    ) -> Result<(), Error> {
        let is_within_period = form_id
            .resolve(self.form_repository)
            .await?
            .and_then(|form| {
                let response_period = form.settings.response_period;

                response_period
                    .start_at
                    .zip(response_period.end_at)
                    .map(|(start_at, end_at)| {
                        let now = Utc::now();
                        now >= start_at && now <= end_at
                    })
            })
            // Note: Noneの場合はフォームが存在していないかそもそも回答期間が無いフォーム
            .unwrap_or(true);

        if is_within_period {
            self.form_repository
                .post_answer(user, form_id, title, answers)
                .await
        } else {
            Err(Error::from(OutOfPeriod))
        }
    }

    pub async fn get_answers(&self, answer_id: AnswerId) -> Result<AnswerDto, Error> {
        if let Some(form_answer) = self.form_repository.get_answers(answer_id).await? {
            let fetch_contents = self.form_repository.get_answer_contents(answer_id);
            let fetch_labels = self
                .form_repository
                .get_labels_for_answers_by_answer_id(answer_id);
            let fetch_comments = self.form_repository.get_comments(answer_id);

            let (contents, labels, comments) =
                try_join!(fetch_contents, fetch_labels, fetch_comments)?;

            Ok(AnswerDto {
                form_answer,
                contents,
                labels,
                comments,
            })
        } else {
            Err(Error::from(AnswerNotFound))
        }
    }

    pub async fn get_answers_by_form_id(&self, form_id: FormId) -> Result<Vec<AnswerDto>, Error> {
        stream::iter(self.form_repository.get_answers_by_form_id(form_id).await?)
            .then(|form_answer| async {
                let fetch_contents = self.form_repository.get_answer_contents(form_answer.id);
                let fetch_labels = self
                    .form_repository
                    .get_labels_for_answers_by_answer_id(form_answer.id);
                let fetch_comments = self.form_repository.get_comments(form_answer.id);

                let (contents, labels, comments) =
                    try_join!(fetch_contents, fetch_labels, fetch_comments)?;

                Ok(AnswerDto {
                    form_answer,
                    contents,
                    labels,
                    comments,
                })
            })
            .collect::<Vec<Result<AnswerDto, Error>>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
    }

    pub async fn get_all_answers(&self) -> Result<Vec<AnswerDto>, Error> {
        stream::iter(self.form_repository.get_all_answers().await?)
            .then(|form_answer| async {
                let fetch_contents = self.form_repository.get_answer_contents(form_answer.id);
                let fetch_labels = self
                    .form_repository
                    .get_labels_for_answers_by_answer_id(form_answer.id);
                let fetch_comments = self.form_repository.get_comments(form_answer.id);

                let (contents, labels, comments) =
                    try_join!(fetch_contents, fetch_labels, fetch_comments)?;

                Ok(AnswerDto {
                    form_answer,
                    contents,
                    labels,
                    comments,
                })
            })
            .collect::<Vec<Result<AnswerDto, Error>>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
    }

    pub async fn update_answer_meta(
        &self,
        answer_id: AnswerId,
        title: Option<String>,
    ) -> Result<(), Error> {
        self.form_repository
            .update_answer_meta(answer_id, title)
            .await
    }

    pub async fn create_questions(
        &self,
        form_id: FormId,
        questions: Vec<Question>,
    ) -> Result<(), Error> {
        self.form_repository
            .create_questions(form_id, questions)
            .await
    }

    pub async fn put_questions(
        &self,
        form_id: FormId,
        questions: Vec<Question>,
    ) -> Result<(), Error> {
        self.form_repository.put_questions(form_id, questions).await
    }

    pub async fn post_comment(&self, comment: Comment, answer_id: AnswerId) -> Result<(), Error> {
        let can_post_comment = match comment.commented_by.role {
            Administrator => true,
            StandardUser => {
                let answer = answer_id
                    .resolve(self.form_repository)
                    .await?
                    .ok_or(AnswerNotFound)?;

                let form = answer
                    .form_id
                    .resolve(self.form_repository)
                    .await?
                    .ok_or(FormNotFound)?;

                form.settings.answer_visibility == PUBLIC
            }
        };

        if can_post_comment {
            self.form_repository.post_comment(answer_id, &comment).await
        } else {
            Err(Error::from(DoNotHavePermissionToPostFormComment))
        }
    }

    pub async fn delete_comment(&self, comment_id: CommentId) -> Result<(), Error> {
        self.form_repository.delete_comment(comment_id).await
    }

    pub async fn create_label_for_answers(&self, label_name: String) -> Result<(), Error> {
        self.form_repository
            .create_label_for_answers(label_name)
            .await
    }

    pub async fn get_labels_for_answers(&self) -> Result<Vec<Label>, Error> {
        self.form_repository.get_labels_for_answers().await
    }

    pub async fn delete_label_for_answers(&self, label_id: LabelId) -> Result<(), Error> {
        self.form_repository
            .delete_label_for_answers(label_id)
            .await
    }

    pub async fn edit_label_for_answers(&self, label_schema: &Label) -> Result<(), Error> {
        self.form_repository
            .edit_label_for_answers(label_schema)
            .await
    }

    pub async fn replace_answer_labels(
        &self,
        answer_id: AnswerId,
        label_ids: Vec<LabelId>,
    ) -> Result<(), Error> {
        self.form_repository
            .replace_answer_labels(answer_id, label_ids)
            .await
    }

    pub async fn create_label_for_forms(&self, label_name: String) -> Result<(), Error> {
        self.form_repository
            .create_label_for_forms(label_name)
            .await
    }

    pub async fn get_labels_for_forms(&self) -> Result<Vec<Label>, Error> {
        self.form_repository.get_labels_for_forms().await
    }

    pub async fn delete_label_for_forms(&self, label_id: LabelId) -> Result<(), Error> {
        self.form_repository.delete_label_for_forms(label_id).await
    }

    pub async fn edit_label_for_forms(&self, label: &Label) -> Result<(), Error> {
        self.form_repository.edit_label_for_forms(label).await
    }

    pub async fn replace_form_labels(
        &self,
        form_id: FormId,
        label_ids: Vec<LabelId>,
    ) -> Result<(), Error> {
        self.form_repository
            .replace_form_labels(form_id, label_ids)
            .await
    }

    pub async fn post_message(
        &self,
        actor: User,
        message_body: String,
        answer_id: AnswerId,
    ) -> Result<(), Error> {
        let form_answer = match self.form_repository.get_answers(answer_id).await? {
            Some(form_answer) => form_answer,
            None => return Err(Error::from(AnswerNotFound)),
        };

        match Message::try_new(form_answer, actor.to_owned(), message_body) {
            Ok(message) => {
                let notification = Notification::new(
                    NotificationSource::Message(message.id().to_owned()),
                    message.related_answer().user.to_owned(),
                );

                let message_sender = message.sender().to_owned();

                let post_message_result = self
                    .form_repository
                    .post_message(&actor, message.into())
                    .await;

                match post_message_result {
                    Ok(_) if message_sender.id != notification.recipient().id => {
                        self.notification_repository.create(&notification).await?;
                        Ok(())
                    }
                    Err(error) => Err(error),
                    _ => Ok(()),
                }
            }
            Err(error) => Err(Error::from(error)),
        }
    }

    pub async fn get_messages(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<AuthorizationGuard<Message, Read>>, Error> {
        let answers = self
            .form_repository
            .get_answers(answer_id)
            .await?
            .ok_or(AnswerNotFound)?;

        self.form_repository
            .fetch_messages_by_answer(&answers)
            .await
    }

    pub async fn update_message_body(
        &self,
        actor: &User,
        answer_id: &AnswerId,
        message_id: &MessageId,
        body: String,
    ) -> Result<(), Error> {
        let message = self
            .form_repository
            .fetch_message(message_id)
            .await?
            .ok_or(MessageNotFound)?;

        if &message.try_read(actor)?.related_answer().id != answer_id {
            return Err(Error::from(MessageNotFound));
        }

        self.form_repository
            .update_message_body(actor, message.into_update(), body)
            .await
    }

    pub async fn delete_message(
        &self,
        actor: &User,
        answer_id: &AnswerId,
        message_id: &MessageId,
    ) -> Result<(), Error> {
        let message = self
            .form_repository
            .fetch_message(message_id)
            .await?
            .ok_or(MessageNotFound)?;

        if &message.try_read(actor)?.related_answer().id != answer_id {
            return Err(Error::from(MessageNotFound));
        }

        self.form_repository
            .delete_message(actor, message.into_delete())
            .await
    }
}
