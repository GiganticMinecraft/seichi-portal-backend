use domain::{
    form::{
        answer::{
            models::{AnswerEntry, AnswerId, FormAnswerContent},
            service::PostAnswerEntriesVerifier,
        },
        models::FormId,
        service::DefaultAnswerTitleDomainService,
    },
    repository::form::{
        answer_label_repository::AnswerLabelRepository, answer_repository::AnswerRepository,
        comment_repository::CommentRepository, form_repository::FormRepository,
        question_repository::QuestionRepository,
    },
    types::verified::Verifier,
    user::models::User,
};
use errors::{usecase::UseCaseError::AnswerNotFound, Error};
use futures::{stream, try_join, StreamExt};

use crate::dto::AnswerDto;

pub struct AnswerUseCase<
    'a,
    AnswerRepo: AnswerRepository,
    FormRepo: FormRepository,
    CommentRepo: CommentRepository,
    AnswerLabelRepo: AnswerLabelRepository,
    QuestionRepo: QuestionRepository,
> {
    pub answer_repository: &'a AnswerRepo,
    pub form_repository: &'a FormRepo,
    pub comment_repository: &'a CommentRepo,
    pub answer_label_repository: &'a AnswerLabelRepo,
    pub question_repository: &'a QuestionRepo,
}

impl<
        R1: AnswerRepository,
        R2: FormRepository,
        R3: CommentRepository,
        R4: AnswerLabelRepository,
        R5: QuestionRepository,
    > AnswerUseCase<'_, R1, R2, R3, R4, R5>
{
    pub async fn post_answers(
        &self,
        user: User,
        form_id: FormId,
        answers: Vec<FormAnswerContent>,
    ) -> Result<(), Error> {
        let form_service = DefaultAnswerTitleDomainService {
            form_repo: self.form_repository,
            question_repo: self.question_repository,
            answer_repo: self.answer_repository,
        };

        let title = form_service
            .to_answer_title(&user, form_id, answers.as_slice())
            .await?;

        let answer_entry = AnswerEntry::new(user.to_owned(), form_id, title);

        let verifier = PostAnswerEntriesVerifier {
            form_repo: self.form_repository,
            actor: &user,
            answer_entry,
        };

        let answer_entry = verifier.verify().await?;

        self.answer_repository
            .post_answer(answer_entry, answers)
            .await
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
            let fetch_contents = self
                .answer_repository
                .get_answer_contents(*form_answer.id());
            let fetch_labels = self
                .answer_label_repository
                .get_labels_for_answers_by_answer_id(*form_answer.id());
            let fetch_comments = self.comment_repository.get_comments(*form_answer.id());

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
                let fetch_contents = self
                    .answer_repository
                    .get_answer_contents(*form_answer.id());
                let fetch_labels = self
                    .answer_label_repository
                    .get_labels_for_answers_by_answer_id(*form_answer.id());
                let fetch_comments = self.comment_repository.get_comments(*form_answer.id());

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
