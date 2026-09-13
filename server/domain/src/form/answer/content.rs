use std::collections::{BTreeSet, HashMap};

use errors::domain::DomainError;
use serde::{Deserialize, Serialize};

use crate::form::{
    models::{FormRevision, FormRevisionId},
    question::QuestionId,
};

pub type FormAnswerContentId = types::Id<FormAnswerContent>;

#[derive(Clone, Serialize, Deserialize, PartialEq, Debug)]
pub struct FormAnswerContent {
    pub id: FormAnswerContentId,
    pub question_id: QuestionId,
    pub answer: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PostedAnswerContents {
    revision_id: FormRevisionId,
    contents: Vec<FormAnswerContent>,
}

impl PostedAnswerContents {
    #[cfg(test)]
    pub(crate) fn for_test(contents: Vec<FormAnswerContent>) -> Self {
        Self {
            revision_id: FormRevisionId::new(),
            contents,
        }
    }

    pub fn try_new(
        revision: &FormRevision,
        contents: Vec<FormAnswerContent>,
    ) -> Result<Self, DomainError> {
        let questions = revision.questions().as_slice();
        let questions_by_id = questions
            .iter()
            .map(|question| (question.id(), question))
            .collect::<HashMap<_, _>>();
        let answered_question_ids = contents
            .iter()
            .map(|answer| answer.question_id)
            .collect::<BTreeSet<_>>();

        if answered_question_ids.len() != contents.len() {
            return Err(DomainError::InvalidEntity {
                message: "duplicate answer for the same question".to_string(),
            });
        }

        if let Some(error) = contents.iter().find_map(|answer| {
            let question = questions_by_id.get(&answer.question_id).ok_or_else(|| {
                DomainError::InvalidEntity {
                    message: format!(
                        "question {} does not belong to the form",
                        answer.question_id
                    ),
                }
            });

            question
                .and_then(|question| question.validate_answer(&answer.answer))
                .err()
        }) {
            return Err(error);
        }

        if let Some(missing_question) = questions
            .iter()
            .filter(|question| question.is_required())
            .find(|question| !answered_question_ids.contains(&question.id()))
        {
            return Err(DomainError::InvalidEntity {
                message: format!(
                    "required question {} is missing",
                    missing_question.template_key().as_str()
                ),
            });
        }

        Ok(Self {
            revision_id: *revision.id(),
            contents,
        })
    }

    pub fn revision_id(&self) -> FormRevisionId {
        self.revision_id
    }

    pub fn as_slice(&self) -> &[FormAnswerContent] {
        &self.contents
    }

    pub fn into_inner(self) -> Vec<FormAnswerContent> {
        self.contents
    }
}

pub(crate) fn parse_multiple_choice_answer(answer: &str) -> Vec<String> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::form::{
        models::FormRevision,
        question::{Choice, Question, QuestionType},
    };
    use types::non_empty_vec::NonEmptyVec;
    use uuid::Uuid;

    fn question_id(seed: &str) -> QuestionId {
        Uuid::parse_str(seed).unwrap().into()
    }

    fn text_question() -> Question {
        unsafe {
            Question::from_raw_parts(
                question_id("00000000-0000-7000-8000-000000000001"),
                "name".to_string().try_into().unwrap(),
                0,
                "Name".to_string().try_into().unwrap(),
                None,
                QuestionType::Text,
                None,
                true,
            )
            .unwrap()
        }
    }

    fn single_choice_question() -> Question {
        unsafe {
            Question::from_raw_parts(
                question_id("00000000-0000-7000-8000-000000000002"),
                "role".to_string().try_into().unwrap(),
                1,
                "Role".to_string().try_into().unwrap(),
                None,
                QuestionType::SingleChoice,
                NonEmptyVec::try_new(vec![
                    Choice::new(Some(1.into()), 0, "Admin".to_string().try_into().unwrap()),
                    Choice::new(Some(2.into()), 1, "User".to_string().try_into().unwrap()),
                ])
                .unwrap()
                .into(),
                true,
            )
            .unwrap()
        }
    }

    fn multiple_choice_question() -> Question {
        unsafe {
            Question::from_raw_parts(
                question_id("00000000-0000-7000-8000-000000000003"),
                "tags".to_string().try_into().unwrap(),
                2,
                "Tags".to_string().try_into().unwrap(),
                None,
                QuestionType::MultipleChoice,
                NonEmptyVec::try_new(vec![
                    Choice::new(
                        Some(3.into()),
                        0,
                        "Admin, Owner".to_string().try_into().unwrap(),
                    ),
                    Choice::new(Some(4.into()), 1, "User".to_string().try_into().unwrap()),
                ])
                .unwrap()
                .into(),
                false,
            )
            .unwrap()
        }
    }

    fn revision(questions: Vec<Question>) -> FormRevision {
        FormRevision::new(
            crate::form::question::QuestionSet::try_new(NonEmptyVec::try_new(questions).unwrap())
                .unwrap(),
        )
    }

    #[test]
    fn posted_answer_contents_rejects_duplicate_question_ids() {
        let questions = vec![text_question()];
        let answers = vec![
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000001"),
                answer: "Alice".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000001"),
                answer: "Bob".to_string(),
            },
        ];

        assert!(PostedAnswerContents::try_new(&revision(questions), answers).is_err());
    }

    #[test]
    fn posted_answer_contents_rejects_question_outside_form() {
        let questions = vec![text_question()];
        let answers = vec![FormAnswerContent {
            id: FormAnswerContentId::new(),
            question_id: question_id("00000000-0000-7000-8000-000000000999"),
            answer: "Alice".to_string(),
        }];

        assert!(PostedAnswerContents::try_new(&revision(questions), answers).is_err());
    }

    #[test]
    fn posted_answer_contents_rejects_invalid_single_choice() {
        let questions = vec![text_question(), single_choice_question()];
        let answers = vec![
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000001"),
                answer: "Alice".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000002"),
                answer: "Guest".to_string(),
            },
        ];

        assert!(PostedAnswerContents::try_new(&revision(questions), answers).is_err());
    }

    #[test]
    fn posted_answer_contents_rejects_invalid_multiple_choice_values() {
        let questions = vec![
            text_question(),
            single_choice_question(),
            multiple_choice_question(),
        ];
        let answers = vec![
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000001"),
                answer: "Alice".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000002"),
                answer: "Admin".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000003"),
                answer: r#"["Admin","Guest"]"#.to_string(),
            },
        ];

        assert!(PostedAnswerContents::try_new(&revision(questions), answers).is_err());
    }

    #[test]
    fn posted_answer_contents_rejects_empty_multiple_choice_values() {
        let questions = vec![
            text_question(),
            single_choice_question(),
            multiple_choice_question(),
        ];
        let answers = vec![
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000001"),
                answer: "Alice".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000002"),
                answer: "Admin".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000003"),
                answer: "[]".to_string(),
            },
        ];

        assert!(PostedAnswerContents::try_new(&revision(questions), answers).is_err());
    }

    #[test]
    fn posted_answer_contents_rejects_missing_required_question() {
        let questions = vec![text_question(), single_choice_question()];
        let answers = vec![FormAnswerContent {
            id: FormAnswerContentId::new(),
            question_id: question_id("00000000-0000-7000-8000-000000000001"),
            answer: "Alice".to_string(),
        }];

        assert!(PostedAnswerContents::try_new(&revision(questions), answers).is_err());
    }

    #[test]
    fn posted_answer_contents_preserves_valid_answers() {
        let questions = vec![
            text_question(),
            single_choice_question(),
            multiple_choice_question(),
        ];
        let answers = vec![
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000001"),
                answer: "Alice".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000002"),
                answer: "Admin".to_string(),
            },
            FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: question_id("00000000-0000-7000-8000-000000000003"),
                answer: r#"["Admin, Owner","User"]"#.to_string(),
            },
        ];

        let posted_answers =
            PostedAnswerContents::try_new(&revision(questions), answers.clone()).unwrap();

        assert_eq!(posted_answers.as_slice(), answers.as_slice());
        assert_eq!(posted_answers.into_inner(), answers);
    }

    #[test]
    fn parse_multiple_choice_answer_accepts_json_with_commas_in_values() {
        assert_eq!(
            parse_multiple_choice_answer(r#"["Admin, Owner","User"]"#),
            vec!["Admin, Owner".to_string(), "User".to_string()]
        );
    }

    #[test]
    fn parse_multiple_choice_answer_falls_back_to_legacy_csv_format() {
        assert_eq!(
            parse_multiple_choice_answer("Admin, User"),
            vec!["Admin".to_string(), "User".to_string()]
        );
    }
}
