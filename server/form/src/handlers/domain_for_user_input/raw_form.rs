use crate::domain::{Form, FormId, FormName, Question};
use crate::handlers::domain_for_user_input::raw_question::RawQuestion;
use derive_getters::Getters;
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Serialize, Deserialize, Getters)]
pub struct RawForm {
    pub form_name: String,
    pub questions: Vec<RawQuestion>,
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
            .id(FormId::builder()
                .form_id(form_id.to_owned().to_owned())
                .build())
            .name(FormName::builder().name(self.form_name.to_owned()).build())
            .questions(questions)
            .build()
    }
}
