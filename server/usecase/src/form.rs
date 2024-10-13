use chrono::Utc;
use domain::{
    form::models::{
        Answer, AnswerId, Comment, CommentId, DefaultAnswerTitle, Form, FormDescription, FormId,
        FormTitle, Label, LabelId, LabelSchema, OffsetAndLimit, PostedAnswers, Question,
        ResponsePeriod, SimpleForm, Visibility, Visibility::PUBLIC, WebhookUrl,
    },
    repository::form_repository::FormRepository,
    user::models::{
        Role::{Administrator, StandardUser},
        User,
    },
};
use errors::{
    usecase::UseCaseError::{
        AnswerNotFound, DoNotHavePermissionToPostFormComment, FormNotFound, OutOfPeriod,
    },
    Error,
};
use futures::future::{join_all, OptionFuture};
use types::Resolver;

pub struct FormUseCase<'a, FormRepo: FormRepository> {
    pub repository: &'a FormRepo,
}

impl<R: FormRepository> FormUseCase<'_, R> {
    pub async fn create_form(
        &self,
        title: FormTitle,
        description: FormDescription,
        user: User,
    ) -> Result<FormId, Error> {
        self.repository.create(title, description, user).await
    }

    pub async fn public_form_list(
        &self,
        offset_and_limit: OffsetAndLimit,
    ) -> Result<Vec<SimpleForm>, Error> {
        self.repository.public_list(offset_and_limit).await
    }

    pub async fn form_list(
        &self,
        offset_and_limit: OffsetAndLimit,
    ) -> Result<Vec<SimpleForm>, Error> {
        self.repository.list(offset_and_limit).await
    }

    pub async fn get_form(&self, form_id: FormId) -> Result<Form, Error> {
        self.repository.get(form_id).await
    }

    pub async fn delete_form(&self, form_id: FormId) -> Result<(), Error> {
        self.repository.delete(form_id).await
    }

    pub async fn get_questions(&self, form_id: FormId) -> Result<Vec<Question>, Error> {
        self.repository.get_questions(form_id).await
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
            .map(|title| self.repository.update_title(form_id, title))
            .into();
        let update_description: OptionFuture<_> = description
            .map(|description| self.repository.update_description(form_id, description))
            .into();
        let update_response_period: OptionFuture<_> = if has_response_period.unwrap_or(false) {
            response_period
                .map(|response_period| {
                    self.repository
                        .update_response_period(form_id, response_period)
                })
                .into()
        } else {
            None.into()
        };
        let update_webhook: OptionFuture<_> = webhook
            .map(|webhook| self.repository.update_webhook_url(form_id, webhook))
            .into();
        let update_default_answer_title: OptionFuture<_> = default_answer_title
            .map(|default_answer_title| {
                self.repository
                    .update_default_answer_title(form_id, default_answer_title)
            })
            .into();
        let update_visibility: OptionFuture<_> = visibility
            .map(|visibility| self.repository.update_visibility(form_id, visibility))
            .into();
        let update_answer_visibility: OptionFuture<_> = answer_visibility
            .map(|visibility| {
                self.repository
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
        answers: Vec<Answer>,
    ) -> Result<(), Error> {
        let is_within_period = form_id
            .resolve(self.repository)
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
            self.repository
                .post_answer(user, form_id, title, answers)
                .await
        } else {
            Err(Error::from(OutOfPeriod))
        }
    }

    pub async fn get_answers(&self, answer_id: AnswerId) -> Result<PostedAnswers, Error> {
        if let Some(posted_answers) = self.repository.get_answers(answer_id).await? {
            Ok(posted_answers)
        } else {
            Err(Error::from(AnswerNotFound))
        }
    }

    pub async fn get_answers_by_form_id(
        &self,
        form_id: FormId,
    ) -> Result<Vec<PostedAnswers>, Error> {
        self.repository.get_answers_by_form_id(form_id).await
    }

    pub async fn get_all_answers(&self) -> Result<Vec<PostedAnswers>, Error> {
        self.repository.get_all_answers().await
    }

    pub async fn update_answer_meta(
        &self,
        answer_id: AnswerId,
        title: Option<String>,
    ) -> Result<(), Error> {
        self.repository.update_answer_meta(answer_id, title).await
    }

    pub async fn create_questions(
        &self,
        form_id: FormId,
        questions: Vec<Question>,
    ) -> Result<(), Error> {
        self.repository.create_questions(form_id, questions).await
    }

    pub async fn put_questions(
        &self,
        form_id: FormId,
        questions: Vec<Question>,
    ) -> Result<(), Error> {
        self.repository.put_questions(form_id, questions).await
    }

    pub async fn post_comment(&self, comment: Comment, answer_id: AnswerId) -> Result<(), Error> {
        let can_post_comment = match comment.commented_by.role {
            Administrator => true,
            StandardUser => {
                let answer = answer_id
                    .resolve(self.repository)
                    .await?
                    .ok_or(AnswerNotFound)?;

                let form = answer
                    .form_id
                    .resolve(self.repository)
                    .await?
                    .ok_or(FormNotFound)?;

                form.settings.answer_visibility == PUBLIC
            }
        };

        if can_post_comment {
            self.repository.post_comment(answer_id, &comment).await
        } else {
            Err(Error::from(DoNotHavePermissionToPostFormComment))
        }
    }

    pub async fn delete_comment(&self, comment_id: CommentId) -> Result<(), Error> {
        self.repository.delete_comment(comment_id).await
    }

    pub async fn create_label_for_answers(&self, label: &LabelSchema) -> Result<(), Error> {
        self.repository.create_label_for_answers(label).await
    }

    pub async fn get_labels_for_answers(&self) -> Result<Vec<Label>, Error> {
        self.repository.get_labels_for_answers().await
    }

    pub async fn delete_label_for_answers(&self, label_id: LabelId) -> Result<(), Error> {
        self.repository.delete_label_for_answers(label_id).await
    }

    pub async fn edit_label_for_answers(&self, label_schema: &Label) -> Result<(), Error> {
        self.repository.edit_label_for_answers(label_schema).await
    }

    pub async fn replace_answer_labels(
        &self,
        answer_id: AnswerId,
        label_ids: Vec<LabelId>,
    ) -> Result<(), Error> {
        self.repository
            .replace_answer_labels(answer_id, label_ids)
            .await
    }

    pub async fn create_label_for_forms(&self, label: &LabelSchema) -> Result<(), Error> {
        self.repository.create_label_for_forms(label).await
    }

    pub async fn get_labels_for_forms(&self) -> Result<Vec<Label>, Error> {
        self.repository.get_labels_for_forms().await
    }

    pub async fn delete_label_for_forms(&self, label_id: LabelId) -> Result<(), Error> {
        self.repository.delete_label_for_forms(label_id).await
    }

    pub async fn edit_label_for_forms(&self, label: &Label) -> Result<(), Error> {
        self.repository.edit_label_for_forms(label).await
    }

    pub async fn replace_form_labels(
        &self,
        form_id: FormId,
        label_ids: Vec<LabelId>,
    ) -> Result<(), Error> {
        self.repository
            .replace_form_labels(form_id, label_ids)
            .await
    }
}
