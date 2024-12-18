use chrono::Utc;
use domain::{
    form::{
        answer::models::{AnswerId, FormAnswerContent},
        models::{DefaultAnswerTitle, FormId},
    },
    repository::form::{
        answer_label_repository::AnswerLabelRepository, answer_repository::AnswerRepository,
        comment_repository::CommentRepository, form_repository::FormRepository,
    },
    user::models::User,
};
use errors::{
    usecase::UseCaseError::{AnswerNotFound, OutOfPeriod},
    Error,
};
use futures::{stream, try_join, StreamExt};

use crate::dto::AnswerDto;

pub struct AnswerUseCase<
    'a,
    AnswerRepo: AnswerRepository,
    FormRepo: FormRepository,
    CommentRepo: CommentRepository,
    AnswerLabelRepo: AnswerLabelRepository,
> {
    pub answer_repository: &'a AnswerRepo,
    pub form_repository: &'a FormRepo,
    pub comment_repository: &'a CommentRepo,
    pub answer_label_repository: &'a AnswerLabelRepo,
}

impl<
        R1: AnswerRepository,
        R2: FormRepository,
        R3: CommentRepository,
        R4: AnswerLabelRepository,
    > AnswerUseCase<'_, R1, R2, R3, R4>
{
    pub async fn post_answers(
        &self,
        user: &User,
        form_id: FormId,
        title: DefaultAnswerTitle,
        answers: Vec<FormAnswerContent>,
    ) -> Result<(), Error> {
        // TODO: answers に対して AuthorizationGuard を実装する必要がある
        let is_within_period = self
            .form_repository
            .get(form_id)
            .await?
            .map(|form| form.try_into_read(user))
            .transpose()?
            .and_then(|form| {
                let response_period = form.settings().response_period();

                response_period
                    .start_at()
                    .to_owned()
                    .zip(response_period.end_at().to_owned())
                    .map(|(start_at, end_at)| {
                        let now = Utc::now();
                        now >= start_at && now <= end_at
                    })
            })
            // Note: Noneの場合はフォームが存在していないかそもそも回答期間が無いフォーム
            .unwrap_or(true);
        if is_within_period {
            self.answer_repository
                .post_answer(user, form_id, title, answers)
                .await
        } else {
            Err(Error::from(OutOfPeriod))
        }
    }

    pub async fn get_answers(&self, answer_id: AnswerId) -> Result<AnswerDto, Error> {
        if let Some(form_answer) = self.answer_repository.get_answers(answer_id).await? {
            let fetch_contents = self.answer_repository.get_answer_contents(answer_id);
            let fetch_labels = self
                .answer_label_repository
                .get_labels_for_answers_by_answer_id(answer_id);
            let fetch_comments = self.comment_repository.get_comments(answer_id);

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
        stream::iter(
            self.answer_repository
                .get_answers_by_form_id(form_id)
                .await?,
        )
        .then(|form_answer| async {
            let fetch_contents = self.answer_repository.get_answer_contents(form_answer.id);
            let fetch_labels = self
                .answer_label_repository
                .get_labels_for_answers_by_answer_id(form_answer.id);
            let fetch_comments = self.comment_repository.get_comments(form_answer.id);

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
        stream::iter(self.answer_repository.get_all_answers().await?)
            .then(|form_answer| async {
                let fetch_contents = self.answer_repository.get_answer_contents(form_answer.id);
                let fetch_labels = self
                    .answer_label_repository
                    .get_labels_for_answers_by_answer_id(form_answer.id);
                let fetch_comments = self.comment_repository.get_comments(form_answer.id);

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
        self.answer_repository
            .update_answer_meta(answer_id, title)
            .await
    }
}
