use derive_getters::Getters;
use errors::domain::DomainError;
#[cfg(test)]
use proptest_derive::Arbitrary;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt, str::FromStr};
use strum_macros::EnumString;
use types::non_empty_string::NonEmptyString;
use types::non_empty_vec::NonEmptyVec;

use crate::{
    account::models::Role,
    auth::Actor,
    types::authorization_guard::{AuthorizationGuardDefinitions, AuthorizationRole, SelfGuarded},
};

pub type QuestionId = types::Id<Question>;
pub type ChoiceId = types::IntegerId<Choice>;

#[derive(Serialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct TemplateKey(String);

impl TemplateKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_inner(self) -> String {
        self.0
    }
}

impl TryFrom<String> for TemplateKey {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        if value.is_empty()
            || value.len() > 255
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
            || value == "username"
        {
            return Err(DomainError::InvalidEntity {
                message: "question.template_key must be 1 to 255 ASCII alphanumeric, underscore, or hyphen characters and must not be \"username\"".to_string(),
            });
        }

        Ok(Self(value))
    }
}

impl FromStr for TemplateKey {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        value.to_owned().try_into()
    }
}

impl<'de> Deserialize<'de> for TemplateKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .try_into()
            .map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for TemplateKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
impl proptest::arbitrary::Arbitrary for TemplateKey {
    type Parameters = ();
    type Strategy = proptest::strategy::BoxedStrategy<Self>;

    fn arbitrary_with(_: Self::Parameters) -> Self::Strategy {
        use proptest::strategy::Strategy;

        proptest::string::string_regex("[A-Za-z0-9_-]{1,255}")
            .expect("valid TemplateKey regex")
            .prop_filter("username is reserved", |value| value != "username")
            .prop_map(|value| Self::try_from(value).expect("strategy only generates valid keys"))
            .boxed()
    }
}

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

    /// [`Choice`] を永続化済みのフィールド値から復元します。
    ///
    /// # Safety
    /// 新規作成ではなく、データベースなど信頼できる永続化済みデータの復元にのみ使用してください。
    pub unsafe fn from_raw_parts(
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
    template_key: TemplateKey,
    position: u16,
    title: NonEmptyString,
    description: Option<NonEmptyString>,
    is_required: bool,
}

impl QuestionDefinition {
    pub fn new(
        id: QuestionId,
        template_key: TemplateKey,
        position: u16,
        title: NonEmptyString,
        description: Option<NonEmptyString>,
        is_required: bool,
    ) -> Self {
        Self {
            id,
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

#[cfg_attr(test, derive(Arbitrary))]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct QuestionSet(NonEmptyVec<Question>);

impl QuestionSet {
    pub fn try_new(questions: NonEmptyVec<Question>) -> Result<Self, DomainError> {
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

    pub fn into_inner(self) -> NonEmptyVec<Question> {
        self.0
    }
}

impl Question {
    pub fn new_text(
        template_key: TemplateKey,
        position: u16,
        title: NonEmptyString,
        description: Option<NonEmptyString>,
        is_required: bool,
    ) -> Result<Self, DomainError> {
        Ok(Self::Text(TextQuestion::new(QuestionDefinition::new(
            QuestionId::new(),
            template_key,
            position,
            title,
            description,
            is_required,
        ))))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_single_choice(
        template_key: TemplateKey,
        position: u16,
        title: NonEmptyString,
        description: Option<NonEmptyString>,
        choices: NonEmptyVec<Choice>,
        is_required: bool,
    ) -> Result<Self, DomainError> {
        Ok(Self::SingleChoice(SelectQuestion::try_new(
            QuestionDefinition::new(
                QuestionId::new(),
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
        template_key: TemplateKey,
        position: u16,
        title: NonEmptyString,
        description: Option<NonEmptyString>,
        choices: NonEmptyVec<Choice>,
        is_required: bool,
    ) -> Result<Self, DomainError> {
        Ok(Self::MultipleChoice(SelectQuestion::try_new(
            QuestionDefinition::new(
                QuestionId::new(),
                template_key,
                position,
                title,
                description,
                is_required,
            ),
            choices,
        )?))
    }

    /// [`Question`] を永続化済みのフィールド値から復元します。
    ///
    /// # Safety
    /// 新規作成ではなく、データベースなど信頼できる永続化済みデータの復元にのみ使用してください。
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn from_raw_parts(
        id: QuestionId,
        template_key: TemplateKey,
        position: u16,
        title: NonEmptyString,
        description: Option<NonEmptyString>,
        question_type: QuestionType,
        choices: Option<NonEmptyVec<Choice>>,
        is_required: bool,
    ) -> Result<Self, DomainError> {
        let definition =
            QuestionDefinition::new(id, template_key, position, title, description, is_required);

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

    pub fn template_key(&self) -> &TemplateKey {
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

    pub fn update_preserving_id(self, updated: Question) -> Result<Self, DomainError> {
        let definition = QuestionDefinition::new(
            self.id(),
            updated.template_key().clone(),
            updated.position(),
            updated.title().clone(),
            updated.description().cloned(),
            updated.is_required(),
        );

        match updated {
            Self::Text(_) => Ok(Self::Text(TextQuestion::new(definition))),
            Self::SingleChoice(question) => Ok(Self::SingleChoice(SelectQuestion::try_new(
                definition,
                question.choices,
            )?)),
            Self::MultipleChoice(question) => Ok(Self::MultipleChoice(SelectQuestion::try_new(
                definition,
                question.choices,
            )?)),
        }
    }
}

impl AuthorizationRole for Question {
    type Role = SelfGuarded;
}

impl AuthorizationGuardDefinitions for Question {
    fn can_create(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::AccountUser(actor) if actor.role() == &Role::Administrator)
    }

    fn can_read(&self, _actor: &Actor) -> bool {
        true
    }

    fn can_update(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::AccountUser(actor) if actor.role() == &Role::Administrator)
    }

    fn can_delete(&self, actor: &Actor) -> bool {
        matches!(actor, Actor::AccountUser(actor) if actor.role() == &Role::Administrator)
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
    use crate::form::question::QuestionType;

    #[test]
    fn template_key_accepts_valid_values() {
        for value in [
            "a",
            "A",
            "0",
            "_",
            "-",
            "question_key-1",
            "Username",
            "USERNAME",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ] {
            assert!(
                TemplateKey::from_str(value).is_ok(),
                "template key {value:?} must be accepted"
            );
        }
    }

    #[test]
    fn template_key_rejects_invalid_values() {
        for value in [
            "",
            "question key",
            "question.key",
            "質問",
            "username",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ] {
            assert!(
                matches!(
                    TemplateKey::from_str(value),
                    Err(DomainError::InvalidEntity { .. })
                ),
                "template key {value:?} must be rejected"
            );
        }
    }

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
        let result = unsafe {
            Question::from_raw_parts(
                Uuid::nil().into(),
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
            )
        };

        assert!(matches!(result, Err(DomainError::InvalidEntity { .. })));
    }

    #[test]
    fn text_question_has_no_choices() {
        let question = Question::new_text(
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
        let questions = NonEmptyVec::try_new(vec![
            Question::new_text(
                "first".to_string().try_into().unwrap(),
                0,
                "Question 1".to_string().try_into().unwrap(),
                None,
                true,
            )
            .unwrap(),
            Question::new_text(
                "second".to_string().try_into().unwrap(),
                1,
                "Question 2".to_string().try_into().unwrap(),
                None,
                false,
            )
            .unwrap(),
        ])
        .unwrap();

        let result = QuestionSet::try_new(questions);

        assert!(result.is_ok());
    }

    #[test]
    fn question_set_rejects_duplicate_position() {
        let questions = NonEmptyVec::try_new(vec![
            Question::new_text(
                "first".to_string().try_into().unwrap(),
                0,
                "Question 1".to_string().try_into().unwrap(),
                None,
                true,
            )
            .unwrap(),
            Question::new_text(
                "second".to_string().try_into().unwrap(),
                0,
                "Question 2".to_string().try_into().unwrap(),
                None,
                false,
            )
            .unwrap(),
        ])
        .unwrap();

        assert!(matches!(
            QuestionSet::try_new(questions),
            Err(DomainError::InvalidEntity { .. })
        ));
    }

    #[test]
    fn question_set_rejects_non_contiguous_position() {
        let questions = NonEmptyVec::try_new(vec![
            Question::new_text(
                "first".to_string().try_into().unwrap(),
                0,
                "Question 1".to_string().try_into().unwrap(),
                None,
                true,
            )
            .unwrap(),
            Question::new_text(
                "second".to_string().try_into().unwrap(),
                2,
                "Question 2".to_string().try_into().unwrap(),
                None,
                false,
            )
            .unwrap(),
        ])
        .unwrap();

        assert!(matches!(
            QuestionSet::try_new(questions),
            Err(DomainError::InvalidEntity { .. })
        ));
    }

    #[test]
    fn question_set_rejects_duplicate_template_keys() {
        let questions = NonEmptyVec::try_new(vec![
            Question::new_text(
                "same".to_string().try_into().unwrap(),
                0,
                "Question 1".to_string().try_into().unwrap(),
                None,
                true,
            )
            .unwrap(),
            Question::new_text(
                "same".to_string().try_into().unwrap(),
                1,
                "Question 2".to_string().try_into().unwrap(),
                None,
                false,
            )
            .unwrap(),
        ])
        .unwrap();

        assert!(matches!(
            QuestionSet::try_new(questions),
            Err(DomainError::InvalidEntity { .. })
        ));
    }
}
