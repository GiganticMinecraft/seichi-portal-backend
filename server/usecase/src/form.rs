use chrono::Utc;
use domain::{
    form::models::{
        Comment, Form, FormDescription, FormId, FormQuestionUpdateSchema, FormTitle,
        FormUpdateTargets, OffsetAndLimit, PostedAnswers, PostedAnswersSchema, Question,
        SimpleForm,
    },
    repository::form_repository::FormRepository,
    user::models::User,
};
use errors::{infra::InfraError::Forbidden, Error};
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

    pub async fn update_form(
        &self,
        form_id: FormId,
        form_update_targets: FormUpdateTargets,
    ) -> Result<(), Error> {
        self.repository.update(form_id, form_update_targets).await
    }

    pub async fn post_answers(
        &self,
        user: &User,
        answers: &PostedAnswersSchema,
    ) -> Result<(), Error> {
        let is_within_period = answers
            .form_id
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
            self.repository.post_answer(user, answers).await
        } else {
            Err(Error::from(Forbidden))
        }
    }

    pub async fn get_all_answers(&self) -> Result<Vec<PostedAnswers>, Error> {
        self.repository.get_all_answers().await
    }

    pub async fn create_questions(
        &self,
        questions: &FormQuestionUpdateSchema,
    ) -> Result<(), Error> {
        self.repository.create_questions(questions).await
    }

    pub async fn put_questions(&self, questions: &FormQuestionUpdateSchema) -> Result<(), Error> {
        self.repository.put_questions(questions).await
    }

    pub async fn post_comment(&self, comment: Comment) -> Result<(), Error> {
        let has_permission = self
            .repository
            .has_permission(comment.answer_id, &comment.commented_by)
            .await?;

        if has_permission {
            self.repository.post_comment(&comment).await
        } else {
            Err(Error::from(Forbidden))
        }
    }
}
