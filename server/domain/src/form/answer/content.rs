use std::collections::{BTreeSet, HashMap};

use errors::domain::DomainError;
use serde::{Deserialize, Serialize};

use crate::form::question::{Question, QuestionId};

pub type FormAnswerContentId = types::Id<FormAnswerContent>;

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct FormAnswerContent {
    pub id: FormAnswerContentId,
    pub question_id: QuestionId,
    pub answer: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PostedAnswerContents(Vec<FormAnswerContent>);

impl PostedAnswerContents {
    pub fn try_new(
        questions: &[Question],
        contents: Vec<FormAnswerContent>,
    ) -> Result<Self, DomainError> {
        let questions_by_id = questions
            .iter()
            .map(|question| (question.id().into_inner(), question))
            .collect::<HashMap<_, _>>();
        let answered_question_ids = contents
            .iter()
            .map(|answer| answer.question_id.into_inner())
            .collect::<BTreeSet<_>>();

        if answered_question_ids.len() != contents.len() {
            return Err(DomainError::InvalidEntity {
                message: "duplicate answer for the same question".to_string(),
            });
        }

        if let Some(error) = contents.iter().find_map(|answer| {
            let question = questions_by_id
                .get(&answer.question_id.into_inner())
                .ok_or_else(|| DomainError::InvalidEntity {
                    message: format!(
                        "question {} does not belong to the form",
                        answer.question_id
                    ),
                });

            question
                .and_then(|question| match question {
                    Question::Text(_) => Ok(()),
                    Question::SingleChoice(choice_question) => choice_question
                        .choices()
                        .iter()
                        .any(|choice| choice.label.as_str() == answer.answer.as_str())
                        .then_some(())
                        .ok_or_else(|| DomainError::InvalidEntity {
                            message: format!(
                                "answer for question {} must match one of the available choices",
                                question.template_key().as_str()
                            ),
                        }),
                    Question::MultipleChoice(choice_question) => {
                        let values = parse_multiple_choice_answer(&answer.answer);
                        (!values.is_empty()
                            && values.iter().all(|value| {
                                choice_question
                                    .choices()
                                    .iter()
                                    .any(|choice| choice.label.as_str() == value.as_str())
                            }))
                        .then_some(())
                        .ok_or_else(|| DomainError::InvalidEntity {
                            message: format!(
                                "answer for question {} must reference only existing choices",
                                question.template_key().as_str()
                            ),
                        })
                    }
                })
                .err()
        }) {
            return Err(error);
        }

        if let Some(missing_question) = questions
            .iter()
            .filter(|question| question.is_required())
            .map(|question| (question.id().into_inner(), question))
            .find(|(question_id, _)| !answered_question_ids.contains(question_id))
            .map(|(_, question)| question)
        {
            return Err(DomainError::InvalidEntity {
                message: format!(
                    "required question {} is missing",
                    missing_question.template_key().as_str()
                ),
            });
        }

        Ok(Self(contents))
    }

    pub fn as_slice(&self) -> &[FormAnswerContent] {
        &self.0
    }

    pub fn into_inner(self) -> Vec<FormAnswerContent> {
        self.0
    }
}

pub(super) fn parse_multiple_choice_answer(answer: &str) -> Vec<String> {
    let trimmed = answer.trim();
    if trimmed.starts_with('[')
        && trimmed.ends_with(']')
        && let Ok(values) = serde_json::from_str::<Vec<String>>(trimmed)
    {
        return values
            .into_iter()
            .map(|value| value.trim().to_string())
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
