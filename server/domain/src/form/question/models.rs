use derive_getters::Getters;
use errors::domain::DomainError;
#[cfg(test)]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use strum_macros::EnumString;
use types::non_empty_vec::NonEmptyVec;

use crate::{
    form::models::FormId,
    types::authorization_guard::AuthorizationGuardDefinitions,
    user::models::{Role, User},
};

pub type QuestionId = types::IntegerId<Question>;
pub type ChoiceId = types::IntegerId<Choice>;

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Clone, Getters, Debug, PartialEq)]
pub struct Choice {
    #[serde(default)]
    pub id: Option<ChoiceId>,
    pub position: u16,
    pub label: String,
}

impl Choice {
    pub fn new(id: Option<ChoiceId>, position: u16, label: String) -> Result<Self, DomainError> {
        let choice = Self {
            id,
            position,
            label,
        };
        choice.validate()?;
        Ok(choice)
    }

    pub fn from_raw_parts(
        id: Option<ChoiceId>,
        position: u16,
        label: String,
    ) -> Result<Self, DomainError> {
        Self::new(id, position, label)
    }

    fn validate(&self) -> Result<(), DomainError> {
        if self.label.trim().is_empty() {
            return Err(DomainError::InvalidEntity {
                message: "choice.label must not be empty".to_string(),
            });
        }

        Ok(())
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Clone, Getters, Debug, PartialEq)]
pub struct Question {
    #[serde(default)]
    pub id: Option<QuestionId>,
    pub form_id: FormId,
    pub template_key: String,
    pub position: u16,
    pub title: String,
    pub description: Option<String>,
    pub question_type: QuestionType,
    #[serde(default)]
    pub choices: Option<NonEmptyVec<Choice>>,
    pub is_required: bool,
}

impl Question {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: Option<QuestionId>,
        form_id: FormId,
        template_key: String,
        position: u16,
        title: String,
        description: Option<String>,
        question_type: QuestionType,
        choices: Option<NonEmptyVec<Choice>>,
        is_required: bool,
    ) -> Result<Self, DomainError> {
        let question = Self {
            id,
            form_id,
            template_key,
            position,
            title,
            description,
            question_type,
            choices,
            is_required,
        };
        question.validate()?;
        Ok(question)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_raw_parts(
        id: Option<QuestionId>,
        form_id: FormId,
        template_key: String,
        position: u16,
        title: String,
        description: Option<String>,
        question_type: QuestionType,
        choices: Option<NonEmptyVec<Choice>>,
        is_required: bool,
    ) -> Result<Self, DomainError> {
        Self::new(
            id,
            form_id,
            template_key,
            position,
            title,
            description,
            question_type,
            choices,
            is_required,
        )
    }

    pub fn validate_set(questions: &[Question]) -> Result<(), DomainError> {
        let positions = questions
            .iter()
            .map(|question| question.position)
            .collect::<BTreeSet<_>>();
        if positions.len() != questions.len()
            || positions
                .into_iter()
                .enumerate()
                .any(|(index, position)| position != index as u16)
        {
            return Err(DomainError::InvalidEntity {
                message: "question.position must be contiguous from 0".to_string(),
            });
        }

        let template_keys = questions
            .iter()
            .map(|question| question.template_key.as_str())
            .collect::<BTreeSet<_>>();
        if template_keys.len() != questions.len() {
            return Err(DomainError::InvalidEntity {
                message: "question.template_key must be unique within a form".to_string(),
            });
        }

        Ok(())
    }

    fn validate(&self) -> Result<(), DomainError> {
        if self.template_key.trim().is_empty() {
            return Err(DomainError::InvalidEntity {
                message: "question.template_key must not be empty".to_string(),
            });
        }

        if self.title.trim().is_empty() {
            return Err(DomainError::InvalidEntity {
                message: "question.title must not be empty".to_string(),
            });
        }

        let choice_positions = self
            .choices
            .iter()
            .flat_map(|choices| choices.iter().map(|choice| choice.position))
            .collect::<BTreeSet<_>>();

        match self.question_type {
            QuestionType::Text => {
                if self.choices.is_some() {
                    return Err(DomainError::InvalidEntity {
                        message: "text question must not have choices".to_string(),
                    });
                }
            }
            QuestionType::SingleChoice | QuestionType::MultipleChoice => {
                let Some(choices) = &self.choices else {
                    return Err(DomainError::InvalidEntity {
                        message: "choice question must have at least one choice".to_string(),
                    });
                };

                if choice_positions.len() != choices.len()
                    || choice_positions
                        .into_iter()
                        .enumerate()
                        .any(|(index, position)| position != index as u16)
                {
                    return Err(DomainError::InvalidEntity {
                        message: format!(
                            "choice.position must be contiguous from 0 for question {}",
                            self.template_key
                        ),
                    });
                }
            }
        }

        Ok(())
    }
}

impl AuthorizationGuardDefinitions for Question {
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

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialOrd, PartialEq, Eq, EnumString)]
pub enum QuestionType {
    #[strum(serialize = "Text", serialize = "TEXT", ascii_case_insensitive)]
    Text,
    #[strum(
        serialize = "SingleChoice",
        serialize = "SINGLE",
        serialize = "SINGLE_CHOICE",
        ascii_case_insensitive
    )]
    SingleChoice,
    #[strum(
        serialize = "MultipleChoice",
        serialize = "MULTIPLE",
        serialize = "MULTIPLE_CHOICE",
        ascii_case_insensitive
    )]
    MultipleChoice,
}

impl std::fmt::Display for QuestionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Text => "Text",
            Self::SingleChoice => "SingleChoice",
            Self::MultipleChoice => "MultipleChoice",
        };
        f.write_str(value)
    }
}

impl TryFrom<String> for QuestionType {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        use std::str::FromStr;
        Self::from_str(&value).map_err(Into::into)
    }
}

#[cfg(test)]
mod test {
    use test_case::test_case;
    use uuid::Uuid;

    use super::*;
    use crate::form::question::models::QuestionType;

    #[test_case("TEXT" => Ok(QuestionType::Text); "legacy upper text")]
    #[test_case("text" => Ok(QuestionType::Text); "legacy lower text")]
    #[test_case("Text" => Ok(QuestionType::Text); "new text")]
    #[test_case("SINGLE" => Ok(QuestionType::SingleChoice); "legacy upper single")]
    #[test_case("single" => Ok(QuestionType::SingleChoice); "legacy lower single")]
    #[test_case("SingleChoice" => Ok(QuestionType::SingleChoice); "new single")]
    #[test_case("MULTIPLE" => Ok(QuestionType::MultipleChoice); "legacy upper multiple")]
    #[test_case("multiple" => Ok(QuestionType::MultipleChoice); "legacy lower multiple")]
    #[test_case("MultipleChoice" => Ok(QuestionType::MultipleChoice); "new multiple")]
    fn string_to_question_type(input: &str) -> Result<QuestionType, DomainError> {
        input.to_owned().try_into()
    }

    #[test]
    fn text_question_rejects_choices() {
        let result = Question::new(
            Some(1.into()),
            FormId::from(Uuid::nil()),
            "template".to_string(),
            0,
            "Question".to_string(),
            None,
            QuestionType::Text,
            Some(
                NonEmptyVec::try_new(vec![Choice::new(None, 0, "A".to_string()).unwrap()]).unwrap(),
            ),
            true,
        );

        assert!(matches!(result, Err(DomainError::InvalidEntity { .. })));
    }

    #[test]
    fn validate_question_set_requires_unique_template_keys_and_contiguous_positions() {
        let form_id = FormId::from(Uuid::nil());
        let questions = vec![
            Question::new(
                Some(1.into()),
                form_id,
                "same".to_string(),
                0,
                "Question 1".to_string(),
                None,
                QuestionType::Text,
                None,
                true,
            )
            .unwrap(),
            Question::new(
                Some(2.into()),
                form_id,
                "same".to_string(),
                2,
                "Question 2".to_string(),
                None,
                QuestionType::Text,
                None,
                false,
            )
            .unwrap(),
        ];

        assert!(matches!(
            Question::validate_set(&questions),
            Err(DomainError::InvalidEntity { .. })
        ));
    }
}
