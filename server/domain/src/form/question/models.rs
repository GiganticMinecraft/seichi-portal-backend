use derive_getters::Getters;
use errors::domain::DomainError;
#[cfg(test)]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use strum_macros::EnumString;
use types::non_empty_string::NonEmptyString;
use types::non_empty_vec::NonEmptyVec;

use crate::{
    form::models::FormId,
    types::authorization_guard::AuthorizationGuardDefinitions,
    user::models::{Role, User},
};

pub type QuestionId = types::Id<Question>;
pub type ChoiceId = types::IntegerId<Choice>;

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Clone, Getters, Debug, PartialEq)]
pub struct Choice {
    #[serde(default)]
    pub id: Option<ChoiceId>,
    pub position: u16,
    pub label: NonEmptyString,
}

impl Choice {
    pub fn new(id: Option<ChoiceId>, position: u16, label: NonEmptyString) -> Self {
        Self {
            id,
            position,
            label,
        }
    }

    pub fn from_raw_parts(
        id: Option<ChoiceId>,
        position: u16,
        label: NonEmptyString,
    ) -> Result<Self, DomainError> {
        Ok(Self::new(id, position, label))
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Clone, Getters, Debug, PartialEq)]
pub struct QuestionDefinition {
    id: QuestionId,
    form_id: FormId,
    template_key: NonEmptyString,
    position: u16,
    title: NonEmptyString,
    description: Option<NonEmptyString>,
    is_required: bool,
}

impl QuestionDefinition {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: QuestionId,
        form_id: FormId,
        template_key: NonEmptyString,
        position: u16,
        title: NonEmptyString,
        description: Option<NonEmptyString>,
        is_required: bool,
    ) -> Self {
        Self {
            id,
            form_id,
            template_key,
            position,
            title,
            description,
            is_required,
        }
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Clone, Getters, Debug, PartialEq)]
pub struct TextQuestion {
    #[serde(flatten)]
    definition: QuestionDefinition,
}

impl TextQuestion {
    pub fn new(definition: QuestionDefinition) -> Self {
        Self { definition }
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Clone, Getters, Debug, PartialEq)]
pub struct SelectQuestion {
    #[serde(flatten)]
    definition: QuestionDefinition,
    choices: NonEmptyVec<Choice>,
}

impl SelectQuestion {
    pub fn try_new(
        definition: QuestionDefinition,
        choices: NonEmptyVec<Choice>,
    ) -> Result<Self, DomainError> {
        let choice_positions = choices
            .iter()
            .map(|choice| choice.position)
            .collect::<BTreeSet<_>>();

        if choice_positions.len() != choices.len()
            || choice_positions
                .into_iter()
                .enumerate()
                .any(|(index, position)| position != index as u16)
        {
            return Err(DomainError::InvalidEntity {
                message: format!(
                    "choice.position must be contiguous from 0 for question {}",
                    definition.template_key.as_str()
                ),
            });
        }

        Ok(Self {
            definition,
            choices,
        })
    }
}

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum Question {
    Text(TextQuestion),
    SingleChoice(SelectQuestion),
    MultipleChoice(SelectQuestion),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct QuestionSet(Vec<Question>);

impl QuestionSet {
    pub fn try_new(questions: Vec<Question>) -> Result<Self, DomainError> {
        let positions = questions
            .iter()
            .map(|question| question.position())
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
            .map(|question| question.template_key().as_str())
            .collect::<BTreeSet<_>>();
        if template_keys.len() != questions.len() {
            return Err(DomainError::InvalidEntity {
                message: "question.template_key must be unique within a form".to_string(),
            });
        }

        Ok(Self(questions))
    }

    pub fn iter(&self) -> impl Iterator<Item = &Question> {
        self.0.iter()
    }

    pub fn as_slice(&self) -> &[Question] {
        &self.0
    }

    pub fn into_inner(self) -> Vec<Question> {
        self.0
    }
}

impl Question {
    #[allow(clippy::too_many_arguments)]
    pub fn new_text(
        form_id: FormId,
        template_key: NonEmptyString,
        position: u16,
        title: NonEmptyString,
        description: Option<NonEmptyString>,
        is_required: bool,
    ) -> Result<Self, DomainError> {
        Ok(Self::Text(TextQuestion::new(QuestionDefinition::new(
            QuestionId::new(),
            form_id,
            template_key,
            position,
            title,
            description,
            is_required,
        ))))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_single_choice(
        form_id: FormId,
        template_key: NonEmptyString,
        position: u16,
        title: NonEmptyString,
        description: Option<NonEmptyString>,
        choices: NonEmptyVec<Choice>,
        is_required: bool,
    ) -> Result<Self, DomainError> {
        Ok(Self::SingleChoice(SelectQuestion::try_new(
            QuestionDefinition::new(
                QuestionId::new(),
                form_id,
                template_key,
                position,
                title,
                description,
                is_required,
            ),
            choices,
        )?))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_multiple_choice(
        form_id: FormId,
        template_key: NonEmptyString,
        position: u16,
        title: NonEmptyString,
        description: Option<NonEmptyString>,
        choices: NonEmptyVec<Choice>,
        is_required: bool,
    ) -> Result<Self, DomainError> {
        Ok(Self::MultipleChoice(SelectQuestion::try_new(
            QuestionDefinition::new(
                QuestionId::new(),
                form_id,
                template_key,
                position,
                title,
                description,
                is_required,
            ),
            choices,
        )?))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_raw_parts(
        id: QuestionId,
        form_id: FormId,
        template_key: NonEmptyString,
        position: u16,
        title: NonEmptyString,
        description: Option<NonEmptyString>,
        question_type: QuestionType,
        choices: Option<NonEmptyVec<Choice>>,
        is_required: bool,
    ) -> Result<Self, DomainError> {
        let definition = QuestionDefinition::new(
            id,
            form_id,
            template_key,
            position,
            title,
            description,
            is_required,
        );

        match question_type {
            QuestionType::Text => {
                if choices.is_some() {
                    return Err(DomainError::InvalidEntity {
                        message: "text question must not have choices".to_string(),
                    });
                }
                Ok(Self::Text(TextQuestion::new(definition)))
            }
            QuestionType::SingleChoice => {
                let Some(choices) = choices else {
                    return Err(DomainError::InvalidEntity {
                        message: "choice question must have at least one choice".to_string(),
                    });
                };
                Ok(Self::SingleChoice(SelectQuestion::try_new(
                    definition, choices,
                )?))
            }
            QuestionType::MultipleChoice => {
                let Some(choices) = choices else {
                    return Err(DomainError::InvalidEntity {
                        message: "choice question must have at least one choice".to_string(),
                    });
                };
                Ok(Self::MultipleChoice(SelectQuestion::try_new(
                    definition, choices,
                )?))
            }
        }
    }

    pub fn definition(&self) -> &QuestionDefinition {
        match self {
            Self::Text(question) => &question.definition,
            Self::SingleChoice(question) | Self::MultipleChoice(question) => &question.definition,
        }
    }

    pub fn id(&self) -> QuestionId {
        *self.definition().id()
    }

    pub fn form_id(&self) -> &FormId {
        self.definition().form_id()
    }

    pub fn template_key(&self) -> &NonEmptyString {
        self.definition().template_key()
    }

    pub fn position(&self) -> u16 {
        *self.definition().position()
    }

    pub fn title(&self) -> &NonEmptyString {
        self.definition().title()
    }

    pub fn description(&self) -> Option<&NonEmptyString> {
        self.definition().description().as_ref()
    }

    pub fn is_required(&self) -> bool {
        *self.definition().is_required()
    }

    pub fn question_type(&self) -> QuestionType {
        match self {
            Self::Text(_) => QuestionType::Text,
            Self::SingleChoice(_) => QuestionType::SingleChoice,
            Self::MultipleChoice(_) => QuestionType::MultipleChoice,
        }
    }

    pub fn choices(&self) -> Option<&NonEmptyVec<Choice>> {
        match self {
            Self::Text(_) => None,
            Self::SingleChoice(question) | Self::MultipleChoice(question) => {
                Some(&question.choices)
            }
        }
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
        let result = Question::from_raw_parts(
            Uuid::nil().into(),
            FormId::from(Uuid::nil()),
            "template".to_string().try_into().unwrap(),
            0,
            "Question".to_string().try_into().unwrap(),
            None,
            QuestionType::Text,
            Some(
                NonEmptyVec::try_new(vec![Choice::new(
                    None,
                    0,
                    "A".to_string().try_into().unwrap(),
                )])
                .unwrap(),
            ),
            true,
        );

        assert!(matches!(result, Err(DomainError::InvalidEntity { .. })));
    }

    #[test]
    fn text_question_has_no_choices() {
        let question = Question::new_text(
            FormId::from(Uuid::nil()),
            "template".to_string().try_into().unwrap(),
            0,
            "Question".to_string().try_into().unwrap(),
            None,
            true,
        )
        .unwrap();

        assert!(matches!(question, Question::Text(_)));
        assert!(question.choices().is_none());
    }

    #[test]
    fn single_choice_question_requires_contiguous_choice_positions() {
        let result = Question::new_single_choice(
            FormId::from(Uuid::nil()),
            "template".to_string().try_into().unwrap(),
            0,
            "Question".to_string().try_into().unwrap(),
            None,
            NonEmptyVec::try_new(vec![
                Choice::new(None, 0, "A".to_string().try_into().unwrap()),
                Choice::new(None, 2, "B".to_string().try_into().unwrap()),
            ])
            .unwrap(),
            true,
        );

        assert!(matches!(result, Err(DomainError::InvalidEntity { .. })));
    }

    #[test]
    fn multiple_choice_question_requires_contiguous_choice_positions() {
        let result = Question::new_multiple_choice(
            FormId::from(Uuid::nil()),
            "template".to_string().try_into().unwrap(),
            0,
            "Question".to_string().try_into().unwrap(),
            None,
            NonEmptyVec::try_new(vec![
                Choice::new(None, 1, "A".to_string().try_into().unwrap()),
                Choice::new(None, 2, "B".to_string().try_into().unwrap()),
            ])
            .unwrap(),
            true,
        );

        assert!(matches!(result, Err(DomainError::InvalidEntity { .. })));
    }

    #[test]
    fn question_set_accepts_unique_template_keys_and_contiguous_positions() {
        let form_id = FormId::from(Uuid::nil());
        let questions = vec![
            Question::new_text(
                form_id,
                "first".to_string().try_into().unwrap(),
                0,
                "Question 1".to_string().try_into().unwrap(),
                None,
                true,
            )
            .unwrap(),
            Question::new_text(
                form_id,
                "second".to_string().try_into().unwrap(),
                1,
                "Question 2".to_string().try_into().unwrap(),
                None,
                false,
            )
            .unwrap(),
        ];

        let result = QuestionSet::try_new(questions);

        assert!(result.is_ok());
    }

    #[test]
    fn question_set_rejects_duplicate_position() {
        let form_id = FormId::from(Uuid::nil());
        let questions = vec![
            Question::new_text(
                form_id,
                "first".to_string().try_into().unwrap(),
                0,
                "Question 1".to_string().try_into().unwrap(),
                None,
                true,
            )
            .unwrap(),
            Question::new_text(
                form_id,
                "second".to_string().try_into().unwrap(),
                0,
                "Question 2".to_string().try_into().unwrap(),
                None,
                false,
            )
            .unwrap(),
        ];

        assert!(matches!(
            QuestionSet::try_new(questions),
            Err(DomainError::InvalidEntity { .. })
        ));
    }

    #[test]
    fn question_set_rejects_non_contiguous_position() {
        let form_id = FormId::from(Uuid::nil());
        let questions = vec![
            Question::new_text(
                form_id,
                "first".to_string().try_into().unwrap(),
                0,
                "Question 1".to_string().try_into().unwrap(),
                None,
                true,
            )
            .unwrap(),
            Question::new_text(
                form_id,
                "second".to_string().try_into().unwrap(),
                2,
                "Question 2".to_string().try_into().unwrap(),
                None,
                false,
            )
            .unwrap(),
        ];

        assert!(matches!(
            QuestionSet::try_new(questions),
            Err(DomainError::InvalidEntity { .. })
        ));
    }

    #[test]
    fn question_set_rejects_duplicate_template_keys() {
        let form_id = FormId::from(Uuid::nil());
        let questions = vec![
            Question::new_text(
                form_id,
                "same".to_string().try_into().unwrap(),
                0,
                "Question 1".to_string().try_into().unwrap(),
                None,
                true,
            )
            .unwrap(),
            Question::new_text(
                form_id,
                "same".to_string().try_into().unwrap(),
                1,
                "Question 2".to_string().try_into().unwrap(),
                None,
                false,
            )
            .unwrap(),
        ];

        assert!(matches!(
            QuestionSet::try_new(questions),
            Err(DomainError::InvalidEntity { .. })
        ));
    }
}
