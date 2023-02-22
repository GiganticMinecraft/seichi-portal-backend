use crate::domain::{Form, FormId, FormName, Question, QuestionType};
use derive_getters::Getters;
use serde::{Deserialize, Serialize};
use strum_macros::Display;
use typed_builder::TypedBuilder;

#[derive(Serialize, Deserialize, Getters)]
pub struct RawForm {
    form_name: String,
    questions: Vec<RawQuestion>,
}

impl RawForm {
    pub fn to_form(&self, form_id: i32) -> Form {
        let questions = self
            .questions
            .iter()
            .map(|question| {
                let question_type = question.question_type.to_question_type();
                Question::builder()
                    .title(question.title().to_owned())
                    .description(question.description().to_owned())
                    .question_type(question_type)
                    .choices(question.choices().to_owned())
                    .build()
            })
            .collect::<Vec<Question>>();

        Form::builder()
            .id(FormId(form_id.to_owned().to_owned()))
            .name(FormName(self.form_name.to_owned()))
            .questions(questions)
            .build()
    }
}

#[derive(Serialize, Deserialize, Getters, TypedBuilder)]
pub struct RawFormId {
    id: i32,
}

#[derive(Serialize, Deserialize, Getters)]
pub struct RawQuestion {
    title: String,
    description: String,
    question_type: RawQuestionType,
    choices: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Display)]
pub enum RawQuestionType {
    #[strum(serialize = "text")]
    TEXT,
    #[strum(serialize = "pulldown")]
    PULLDOWN,
    #[strum(serialize = "checkbox")]
    CHECKBOX,
}

impl RawQuestionType {
    pub fn to_question_type(&self) -> QuestionType {
        match self {
            RawQuestionType::TEXT => QuestionType::TEXT,
            RawQuestionType::CHECKBOX => QuestionType::CHECKBOX,
            RawQuestionType::PULLDOWN => QuestionType::PULLDOWN,
        }
    }
}
