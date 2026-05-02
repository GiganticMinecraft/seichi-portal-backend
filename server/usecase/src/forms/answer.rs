use domain::{
    form::{
        answer::{
            models::{AnswerEntry, AnswerId, AnswerTitle, FormAnswerContent},
            service::AnswerEntryAuthorizationContext,
        },
        comment::service::CommentAuthorizationContext,
        models::FormId,
        question::models::{Question, QuestionType},
        service::DefaultAnswerTitleDomainService,
    },
    repository::form::{
        answer_label_repository::AnswerLabelRepository, answer_repository::AnswerRepository,
        comment_repository::CommentRepository, form_repository::FormRepository,
        question_repository::QuestionRepository,
    },
    types::authorization_guard_with_context::AuthorizationGuardWithContext,
    user::models::User,
};
use errors::{
    Error,
    domain::DomainError,
    usecase::UseCaseError::{AnswerNotFound, FormNotFound},
};
use futures::{StreamExt, stream, try_join};
use std::collections::{BTreeSet, HashMap};

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

        let (form, question_guards, title) = try_join!(
            self.form_repository.get(form_id),
            self.question_repository.get_questions(form_id),
            form_service.to_answer_title(&user, form_id, answers.as_slice())
        )?;

        let form = form.ok_or(Error::from(FormNotFound))?;
        let questions = question_guards
            .into_iter()
            .map(|question| question.try_into_read(&user))
            .collect::<Result<Vec<_>, _>>()?;
        validate_posted_answers(&questions, &answers)?;

        let form_settings = form.try_read(&user)?.settings();

        let answer_entry = AnswerEntry::new(user.to_owned(), form_id, title, answers);
        let context = AnswerEntryAuthorizationContext {
            form_visibility: form_settings.visibility().to_owned(),
            response_period: form_settings.answer_settings().response_period().to_owned(),
            answer_visibility: form_settings.answer_settings().visibility().to_owned(),
        };

        let guard = AuthorizationGuardWithContext::new(answer_entry);

        self.answer_repository
            .post_answer(&context, guard, &user)
            .await
    }

    pub async fn get_answers(
        &self,
        form_id: FormId,
        answer_id: AnswerId,
        user: &User,
    ) -> Result<AnswerDto, Error> {
        if let Some(form_answer_guard) = self.answer_repository.get_answer(answer_id).await? {
            let form_guard = self
                .form_repository
                .get(form_id)
                .await?
                .ok_or(FormNotFound)?;

            let form = form_guard.try_read(user)?;
            let form_settings = form.settings();

            let context = AnswerEntryAuthorizationContext {
                form_visibility: form_settings.visibility().to_owned(),
                response_period: form_settings.answer_settings().response_period().to_owned(),
                answer_visibility: form_settings.answer_settings().visibility().to_owned(),
            };

            let fetch_labels = self
                .answer_label_repository
                .get_labels_for_answers_by_answer_id(answer_id);
            let fetch_comments = self.comment_repository.get_comments(answer_id);

            let (labels, comments) = try_join!(fetch_labels, fetch_comments)?;

            let comment_authorization_context = CommentAuthorizationContext {
                related_answer_entry_guard: form_answer_guard,
                related_answer_entry_guard_context: context,
            };

            let comments = comments
                .into_iter()
                .map(|comment| comment.try_into_read(user, &comment_authorization_context))
                .collect::<Result<Vec<_>, _>>()?;

            let form_answer = comment_authorization_context
                .related_answer_entry_guard
                .try_into_read(
                    user,
                    &comment_authorization_context.related_answer_entry_guard_context,
                )?;

            let labels = labels
                .into_iter()
                .map(|label| label.try_into_read(user))
                .collect::<Result<Vec<_>, _>>()?;

            Ok(AnswerDto {
                form_answer,
                labels,
                comments,
            })
        } else {
            Err(Error::from(AnswerNotFound))
        }
    }

    pub async fn get_answers_by_form_id(
        &self,
        form_id: FormId,
        actor: &User,
    ) -> Result<Vec<AnswerDto>, Error> {
        let form = self
            .form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?;

        let form_settings = form.try_read(actor)?.settings();

        let context = AnswerEntryAuthorizationContext {
            form_visibility: form_settings.visibility().to_owned(),
            response_period: form_settings.answer_settings().response_period().to_owned(),
            answer_visibility: form_settings.answer_settings().visibility().to_owned(),
        };

        stream::iter(
            self.answer_repository
                .get_answers_by_form_id(form_id)
                .await?,
        )
        .then(|form_answer_guard| async {
            let form_answer = form_answer_guard.try_read(actor, &context)?;

            let fetch_labels = self
                .answer_label_repository
                .get_labels_for_answers_by_answer_id(*form_answer.id());
            let fetch_comments = self.comment_repository.get_comments(*form_answer.id());

            let (labels, comments) = try_join!(fetch_labels, fetch_comments)?;

            let comment_authorization_context = CommentAuthorizationContext {
                related_answer_entry_guard: form_answer_guard,
                related_answer_entry_guard_context: AnswerEntryAuthorizationContext {
                    form_visibility: form_settings.visibility().to_owned(),
                    response_period: form_settings.answer_settings().response_period().to_owned(),
                    answer_visibility: form_settings.answer_settings().visibility().to_owned(),
                },
            };

            let comments = comments
                .into_iter()
                .map(|comment| comment.try_into_read(actor, &comment_authorization_context))
                .collect::<Result<Vec<_>, _>>()?;

            let form_answer = comment_authorization_context
                .related_answer_entry_guard
                .try_into_read(
                    actor,
                    &comment_authorization_context.related_answer_entry_guard_context,
                )?;

            let labels = labels
                .into_iter()
                .map(|label| label.try_into_read(actor))
                .collect::<Result<Vec<_>, _>>()?;

            Ok(AnswerDto {
                form_answer,
                labels,
                comments,
            })
        })
        .collect::<Vec<Result<AnswerDto, Error>>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
    }

    pub async fn get_all_answers(&self, user: &User) -> Result<Vec<AnswerDto>, Error> {
        stream::iter(self.answer_repository.get_all_answers().await?)
            .then(|form_answer_guard| async move {
                let context = form_answer_guard
                    .create_context(move |entry| {
                        let form_id = entry.form_id().to_owned();

                        async move {
                            let guard = self
                                .form_repository
                                .get(form_id)
                                .await?
                                .ok_or(FormNotFound)?;

                            let form = guard.try_read(user)?;
                            let form_settings = form.settings();

                            Ok(AnswerEntryAuthorizationContext {
                                form_visibility: form_settings.visibility().to_owned(),
                                response_period: form_settings
                                    .answer_settings()
                                    .response_period()
                                    .to_owned(),
                                answer_visibility: form_settings
                                    .answer_settings()
                                    .visibility()
                                    .to_owned(),
                            })
                        }
                    })
                    .await?;

                let form_answer = form_answer_guard.try_read(user, &context)?;
                let fetch_labels = self
                    .answer_label_repository
                    .get_labels_for_answers_by_answer_id(*form_answer.id());
                let fetch_comments = self.comment_repository.get_comments(*form_answer.id());

                let (labels, comments) = try_join!(fetch_labels, fetch_comments)?;

                let comment_authorization_context = CommentAuthorizationContext {
                    related_answer_entry_guard: form_answer_guard,
                    related_answer_entry_guard_context: context,
                };

                let comments = comments
                    .into_iter()
                    .map(|comment| comment.try_into_read(user, &comment_authorization_context))
                    .collect::<Result<Vec<_>, _>>()?;

                let form_answer = comment_authorization_context
                    .related_answer_entry_guard
                    .try_into_read(
                        user,
                        &comment_authorization_context.related_answer_entry_guard_context,
                    )?;

                let labels = labels
                    .into_iter()
                    .map(|label| label.try_into_read(user))
                    .collect::<Result<Vec<_>, _>>()?;

                Ok(AnswerDto {
                    form_answer,
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
        form_id: FormId,
        answer_id: AnswerId,
        actor: &User,
        title: Option<AnswerTitle>,
    ) -> Result<AnswerDto, Error> {
        let form_guard = self
            .form_repository
            .get(form_id)
            .await?
            .ok_or(FormNotFound)?;

        let form = form_guard.try_read(actor)?;
        let form_settings = form.settings();

        let context = AnswerEntryAuthorizationContext {
            form_visibility: form_settings.visibility().to_owned(),
            response_period: form_settings.answer_settings().response_period().to_owned(),
            answer_visibility: form_settings.answer_settings().visibility().to_owned(),
        };

        if let Some(title) = title {
            let answer_entry = self
                .answer_repository
                .get_answer(answer_id)
                .await?
                .ok_or(Error::from(AnswerNotFound))?
                .into_update()
                .map(|entry| entry.with_title(title));

            self.answer_repository
                .update_answer_entry(actor, &context, answer_entry)
                .await?;
        }

        let answer_guard = self
            .answer_repository
            .get_answer(answer_id)
            .await?
            .ok_or(Error::from(AnswerNotFound))?;

        let fetch_labels = self
            .answer_label_repository
            .get_labels_for_answers_by_answer_id(answer_id);
        let fetch_comments = self.comment_repository.get_comments(answer_id);

        let (labels, comments) = try_join!(fetch_labels, fetch_comments)?;

        let labels = labels
            .into_iter()
            .map(|label| label.try_into_read(actor))
            .collect::<Result<Vec<_>, _>>()?;

        let comment_authorization_context = CommentAuthorizationContext {
            related_answer_entry_guard: answer_guard,
            related_answer_entry_guard_context: context,
        };

        let comments = comments
            .into_iter()
            .map(|comment| comment.try_into_read(actor, &comment_authorization_context))
            .collect::<Result<Vec<_>, _>>()?;

        Ok(AnswerDto {
            form_answer: comment_authorization_context
                .related_answer_entry_guard
                .try_into_read(
                    actor,
                    &comment_authorization_context.related_answer_entry_guard_context,
                )?,
            labels,
            comments,
        })
    }
}

fn validate_posted_answers(
    questions: &[Question],
    answers: &[FormAnswerContent],
) -> Result<(), Error> {
    let questions_by_id = questions
        .iter()
        .filter_map(|question| question.id.map(|id| (id.into_inner(), question)))
        .collect::<HashMap<_, _>>();
    let answered_question_ids = answers
        .iter()
        .map(|answer| answer.question_id.into_inner())
        .collect::<BTreeSet<_>>();
    if answered_question_ids.len() != answers.len() {
        return Err(DomainError::InvalidEntity {
            message: "duplicate answer for the same question".to_string(),
        }
        .into());
    }

    if let Some(error) = answers.iter().find_map(|answer| {
        let question = questions_by_id
            .get(&answer.question_id.into_inner())
            .ok_or_else(|| DomainError::InvalidEntity {
                message: format!(
                    "question {} does not belong to the form",
                    answer.question_id
                ),
            });

        question
            .and_then(|question| match question.question_type {
                QuestionType::Text => Ok(()),
                QuestionType::SingleChoice => question
                    .choices
                    .iter()
                    .flat_map(|choices| choices.iter())
                    .any(|choice| choice.label == answer.answer)
                    .then_some(())
                    .ok_or_else(|| DomainError::InvalidEntity {
                        message: format!(
                            "answer for question {} must match one of the available choices",
                            question.template_key
                        ),
                    }),
                QuestionType::MultipleChoice => {
                    let values = parse_multiple_choice_answer(&answer.answer);
                    (!values.is_empty()
                        && values.iter().all(|value| {
                            question
                                .choices
                                .iter()
                                .flat_map(|choices| choices.iter())
                                .any(|choice| choice.label == *value)
                        }))
                    .then_some(())
                    .ok_or_else(|| DomainError::InvalidEntity {
                        message: format!(
                            "answer for question {} must reference only existing choices",
                            question.template_key
                        ),
                    })
                }
            })
            .err()
    }) {
        return Err(error.into());
    }

    if let Some(missing_question) = questions
        .iter()
        .filter(|question| question.is_required)
        .filter_map(|question| question.id.map(|id| (id.into_inner(), question)))
        .find(|(question_id, _)| !answered_question_ids.contains(question_id))
        .map(|(_, question)| question)
    {
        return Err(DomainError::InvalidEntity {
            message: format!(
                "required question {} is missing",
                missing_question.template_key
            ),
        }
        .into());
    }

    Ok(())
}

fn parse_multiple_choice_answer(answer: &str) -> Vec<String> {
    let trimmed = answer.trim();
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return trimmed[1..trimmed.len() - 1]
            .split(',')
            .map(|value| value.trim().trim_matches('"').to_string())
            .filter(|value| !value.is_empty())
            .collect();
    }

    trimmed
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::form::{
        answer::models::FormAnswerContentId,
        models::FormId,
        question::models::{Choice, QuestionId},
    };
    use types::non_empty_vec::NonEmptyVec;
    use uuid::Uuid;

    fn text_question() -> Question {
        Question::new(
            Some(QuestionId::from(1)),
            FormId::from(Uuid::nil()),
            "name".to_string(),
            0,
            "Name".to_string(),
            None,
            QuestionType::Text,
            None,
            true,
        )
        .unwrap()
    }

    fn single_choice_question() -> Question {
        Question::new(
            Some(QuestionId::from(2)),
            FormId::from(Uuid::nil()),
            "role".to_string(),
            1,
            "Role".to_string(),
            None,
            QuestionType::SingleChoice,
            Some(
                NonEmptyVec::try_new(vec![
                    Choice::new(Some(1.into()), 0, "Admin".to_string()).unwrap(),
                    Choice::new(Some(2.into()), 1, "User".to_string()).unwrap(),
                ])
                .unwrap(),
            ),
            true,
        )
        .unwrap()
    }

    #[test]
    fn validate_posted_answers_rejects_duplicate_question_ids() {
        let questions = vec![text_question()];
        let answers = vec![
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: QuestionId::from(1),
                answer: "Alice".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: QuestionId::from(1),
                answer: "Bob".to_string(),
            },
        ];

        assert!(validate_posted_answers(&questions, &answers).is_err());
    }

    #[test]
    fn validate_posted_answers_rejects_invalid_single_choice() {
        let questions = vec![text_question(), single_choice_question()];
        let answers = vec![
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: QuestionId::from(1),
                answer: "Alice".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: QuestionId::from(2),
                answer: "Guest".to_string(),
            },
        ];

        assert!(validate_posted_answers(&questions, &answers).is_err());
    }
}
