use domain::form::question::models::Question;
use domain::{
    form::{
        answer::settings::models::{AnswerVisibility, DefaultAnswerTitle, ResponsePeriod},
        models::{Form, FormDescription, FormId, FormLabel, FormTitle, Visibility, WebhookUrl},
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
use errors::{Error, usecase::UseCaseError::FormNotFound};

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
        let form_id = form.id().to_owned();

        self.form_repository.create(&user, form.into()).await?;

        Ok(form_id)
    }

    /// `actor` が参照可能なフォームのリストを取得する
    pub async fn form_list(
        &self,
        actor: &User,
        offset: Option<u32>,
        limit: Option<u32>,
    ) -> Result<Vec<(Form, Vec<Question>, Vec<FormLabel>)>, Error> {
        let forms = self
            .form_repository
            .list(offset, limit)
            .await?
            .into_iter()
            .flat_map(|form| form.try_into_read(actor))
            .collect::<Vec<_>>();

        let form_labels = futures::future::try_join_all(forms.iter().map(|form| {
            self.form_label_repository
                .fetch_labels_by_form_id(*form.id())
        }))
        .await?;

        let questions = futures::future::try_join_all(
            forms
                .iter()
                .map(|form| self.question_repository.get_questions(*form.id())),
        )
        .await?;

        let forms_with_labels = forms
            .into_iter()
            .zip(form_labels)
            .zip(questions)
            .map(|((form, labels), questions)| {
                Ok::<_, Error>((
                    form,
                    questions
                        .into_iter()
                        .map(|question| question.try_into_read(actor))
                        .collect::<Result<Vec<_>, _>>()?,
                    labels
                        .into_iter()
                        .map(|guard| guard.try_into_read(actor))
                        .collect::<Result<Vec<_>, _>>()?,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(forms_with_labels)
    }

    pub async fn get_form(&self, actor: &User, form_id: FormId) -> Result<FormDto, Error> {
        let form = self
            .form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?
            .try_into_read(actor)?;

        let questions = self
            .question_repository
            .get_questions(form_id)
            .await?
            .into_iter()
            .map(|question| question.try_into_read(actor))
            .collect::<Result<Vec<_>, _>>()?;
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

    pub async fn delete_form(&self, actor: &User, form_id: FormId) -> Result<(), Error> {
        let form = self
            .form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?;

        self.form_repository.delete(actor, form.into_delete()).await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn update_form(
        &self,
        actor: &User,
        form_id: FormId,
        title: Option<FormTitle>,
        description: Option<FormDescription>,
        response_period: Option<ResponsePeriod>,
        webhook: Option<WebhookUrl>,
        default_answer_title: Option<DefaultAnswerTitle>,
        visibility: Option<Visibility>,
        answer_visibility: Option<AnswerVisibility>,
    ) -> Result<(), Error> {
        let current_form = self
            .form_repository
            .get(form_id)
            .await?
            .ok_or(Error::from(FormNotFound))?;

        let updated_form = current_form.into_update().map(|form| {
            let current_answer_settings = form.settings().answer_settings().to_owned();
            let updated_answer_settings = match answer_visibility {
                None => current_answer_settings,
                Some(visibility) => current_answer_settings.change_visibility(visibility),
            };
            let updated_answer_settings = match default_answer_title {
                None => updated_answer_settings,
                Some(default_answer_title) => {
                    updated_answer_settings.change_default_answer_title(default_answer_title)
                }
            };
            let updated_answer_settings = match response_period {
                None => updated_answer_settings,
                Some(response_period) => {
                    updated_answer_settings.change_response_period(response_period)
                }
            };

            let current_settings = form.settings().to_owned();
            let updated_settings = match visibility {
                None => current_settings,
                Some(visibility) => current_settings.change_visibility(visibility),
            };
            let updated_settings = match webhook {
                None => updated_settings,
                Some(webhook) => updated_settings.change_webhook_url(webhook),
            };
            let updated_settings = updated_settings.change_answer_settings(updated_answer_settings);

            let updated_form = match title {
                None => form,
                Some(title) => form.change_title(title),
            };
            let updated_form = match description {
                None => updated_form,
                Some(description) => updated_form.change_description(description),
            };
            updated_form.change_settings(updated_settings)
        });

        self.form_repository
            .update_form(actor, updated_form)
            .await?;

        Ok(())
    }
}
