use domain::{
    form::{
        answer::settings::models::{AnswerVisibility, DefaultAnswerTitle, ResponsePeriod},
        models::{Form, FormDescription, FormId, FormTitle, Visibility, WebhookUrl},
    },
    repository::{
        form::{
            form_label_repository::FormLabelRepository, form_repository::FormRepository,
            question_repository::QuestionRepository,
        },
        notification_repository::NotificationRepository,
    },
    user::models::User,
};
use errors::{usecase::UseCaseError::FormNotFound, Error};
use futures::future::{join_all, OptionFuture};

use crate::dto::FormDto;

pub struct FormUseCase<
    'a,
    FormRepo: FormRepository,
    NotificationRepo: NotificationRepository,
    QuestionRepo: QuestionRepository,
    FormLabelRepo: FormLabelRepository,
> {
    pub form_repository: &'a FormRepo,
    pub notification_repository: &'a NotificationRepo,
    pub question_repository: &'a QuestionRepo,
    pub form_label_repository: &'a FormLabelRepo,
}

impl<
        R1: FormRepository,
        R2: NotificationRepository,
        R3: QuestionRepository,
        R4: FormLabelRepository,
    > FormUseCase<'_, R1, R2, R3, R4>
{
    pub async fn create_form(
        &self,
        title: FormTitle,
        description: FormDescription,
        user: User,
    ) -> Result<FormId, Error> {
        let form = Form::new(title, description);

        self.form_repository.create(&form, &user).await?;

        Ok(form.id().to_owned())
    }

    /// `actor` が参照可能なフォームのリストを取得する
    pub async fn form_list(
        &self,
        actor: &User,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Vec<Form>, Error> {
        Ok(self
            .form_repository
            .list(offset, limit)
            .await?
            .into_iter()
            .flat_map(|form| form.try_into_read(actor))
            .collect::<Vec<_>>())
    }

    pub async fn get_form(&self, actor: &User, form_id: FormId) -> Result<FormDto, Error> {
        let form = self
            .form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?
            .try_into_read(actor)?;

        let questions = self.question_repository.get_questions(form_id).await?;
        let labels = self
            .form_label_repository
            .fetch_labels_by_form_id(form_id)
            .await?
            .into_iter()
            .map(|label| label.try_into_read(actor))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(FormDto {
            form,
            questions,
            labels,
        })
    }

    pub async fn delete_form(&self, form_id: FormId) -> Result<(), Error> {
        self.form_repository.delete(form_id).await
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
        answer_visibility: Option<&AnswerVisibility>,
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
}
