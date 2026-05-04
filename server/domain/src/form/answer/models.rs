use chrono::{DateTime, Utc};
use derive_getters::Getters;
use deriving_via::DerivingVia;
use errors::domain::DomainError;
#[cfg(test)]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use types::non_empty_string::NonEmptyString;

use crate::{
    form::{
        models::FormId,
        question::models::{Question, QuestionId},
    },
    types::authorization_guard::AuthorizationGuardDefinitions,
    user::models::{Role, User},
};

pub type AnswerId = types::Id<AnswerEntry>;

#[derive(Clone, DerivingVia, Default, Debug, PartialEq)]
#[deriving(From, Into, IntoInner, Serialize(via: Option::<NonEmptyString>), Deserialize(via: Option::<NonEmptyString>
))]
pub struct AnswerTitle(Option<NonEmptyString>);

impl AnswerTitle {
    pub fn new(title: Option<NonEmptyString>) -> Self {
        Self(title)
    }
}

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

#[derive(Serialize, Deserialize, Getters, PartialEq, Debug)]
pub struct AnswerEntry {
    id: AnswerId,
    user: User,
    timestamp: DateTime<Utc>,
    form_id: FormId,
    title: AnswerTitle,
    contents: Vec<FormAnswerContent>,
}

impl AnswerEntry {
    /// [`AnswerEntry`] を新しく作成します。
    pub fn new(
        user: User,
        form_id: FormId,
        title: AnswerTitle,
        contents: PostedAnswerContents,
    ) -> Self {
        Self {
            id: AnswerId::new(),
            user,
            timestamp: Utc::now(),
            form_id,
            title,
            contents: contents.into_inner(),
        }
    }

    /// [`AnswerEntry`] の各フィールドを指定して新しく作成します。
    ///
    /// # Safety
    /// この関数はオブジェクトを新しく作成しない場合
    /// (例えば、データベースから取得した場合)にのみ使用してください。
    pub unsafe fn from_raw_parts(
        id: AnswerId,
        user: User,
        timestamp: DateTime<Utc>,
        form_id: FormId,
        title: AnswerTitle,
        contents: Vec<FormAnswerContent>,
    ) -> Self {
        Self {
            id,
            user,
            timestamp,
            form_id,
            title,
            contents,
        }
    }

    pub fn with_title(self, title: AnswerTitle) -> Self {
        Self { title, ..self }
    }
}

pub type AnswerLabelId = types::Id<AnswerLabel>;

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Getters, Debug, PartialEq)]
pub struct AnswerLabel {
    id: AnswerLabelId,
    name: NonEmptyString,
}

impl AnswerLabel {
    pub fn new(name: NonEmptyString) -> Self {
        Self {
            id: AnswerLabelId::new(),
            name,
        }
    }

    pub fn from_raw_parts(id: AnswerLabelId, name: NonEmptyString) -> Self {
        Self { id, name }
    }

    pub fn renamed(self, name: NonEmptyString) -> Self {
        Self { name, ..self }
    }
}

impl AuthorizationGuardDefinitions for AnswerLabel {
    fn can_create(&self, actor: &User) -> bool {
        actor.role == Role::Administrator
    }

    fn can_read(&self, _actor: &User) -> bool {
        true
    }

    fn can_update(&self, actor: &User) -> bool {
        actor.role == Role::Administrator
    }

    fn can_delete(&self, actor: &User) -> bool {
        actor.role == Role::Administrator
    }
}

fn parse_multiple_choice_answer(answer: &str) -> Vec<String> {
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
    use crate::form::question::models::Choice;
    use types::non_empty_vec::NonEmptyVec;
    use uuid::Uuid;

    fn question_id(seed: &str) -> QuestionId {
        Uuid::parse_str(seed).unwrap().into()
    }

    fn text_question() -> Question {
        Question::from_raw_parts(
            question_id("00000000-0000-7000-8000-000000000001"),
            "name".to_string().try_into().unwrap(),
            0,
            "Name".to_string().try_into().unwrap(),
            None,
            crate::form::question::models::QuestionType::Text,
            None,
            true,
        )
        .unwrap()
    }

    fn single_choice_question() -> Question {
        Question::from_raw_parts(
            question_id("00000000-0000-7000-8000-000000000002"),
            "role".to_string().try_into().unwrap(),
            1,
            "Role".to_string().try_into().unwrap(),
            None,
            crate::form::question::models::QuestionType::SingleChoice,
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

    fn multiple_choice_question() -> Question {
        Question::from_raw_parts(
            question_id("00000000-0000-7000-8000-000000000003"),
            "tags".to_string().try_into().unwrap(),
            2,
            "Tags".to_string().try_into().unwrap(),
            None,
            crate::form::question::models::QuestionType::MultipleChoice,
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

        assert!(PostedAnswerContents::try_new(&questions, answers).is_err());
    }

    #[test]
    fn posted_answer_contents_rejects_question_outside_form() {
        let questions = vec![text_question()];
        let answers = vec![FormAnswerContent {
            id: FormAnswerContentId::new(),
            question_id: question_id("00000000-0000-7000-8000-000000000999"),
            answer: "Alice".to_string(),
        }];

        assert!(PostedAnswerContents::try_new(&questions, answers).is_err());
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

        assert!(PostedAnswerContents::try_new(&questions, answers).is_err());
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

        assert!(PostedAnswerContents::try_new(&questions, answers).is_err());
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

        assert!(PostedAnswerContents::try_new(&questions, answers).is_err());
    }

    #[test]
    fn posted_answer_contents_rejects_missing_required_question() {
        let questions = vec![text_question(), single_choice_question()];
        let answers = vec![FormAnswerContent {
            id: FormAnswerContentId::new(),
            question_id: question_id("00000000-0000-7000-8000-000000000001"),
            answer: "Alice".to_string(),
        }];

        assert!(PostedAnswerContents::try_new(&questions, answers).is_err());
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

        let posted_answers = PostedAnswerContents::try_new(&questions, answers.clone()).unwrap();

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
