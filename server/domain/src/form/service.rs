use errors::Error;
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::form::{
    answer::models::{AnswerTitle, PostedAnswerContents},
    answer_entry_set::models::DefaultAnswerTitle,
    models::Question,
};

pub struct DefaultAnswerTitleDomainService<FormRepo> {
    _phantom: std::marker::PhantomData<FormRepo>,
}

impl<FormRepo> DefaultAnswerTitleDomainService<FormRepo> {
    pub fn to_answer_title_from_questions(
        default_answer_title: DefaultAnswerTitle,
        questions: &[Question],
        answers: &PostedAnswerContents,
        author_name: &str,
    ) -> Result<AnswerTitle, Error> {
        match default_answer_title.into_inner() {
            Some(default_answer_title) => {
                let default_answer_title = default_answer_title.to_string();
                let question_template_key_by_id = questions
                    .iter()
                    .map(|question| (question.id().into_inner(), question.template_key().as_str()))
                    .collect::<HashMap<_, _>>();
                let answers_by_template_key = answers
                    .as_slice()
                    .iter()
                    .filter_map(|answer| {
                        question_template_key_by_id
                            .get(&answer.question_id.into_inner())
                            .map(|template_key| (*template_key, answer.answer.as_str()))
                    })
                    .collect::<HashMap<_, _>>();

                let answer_replaced_title: String = question_placeholder_regex()
                    .replace_all(default_answer_title.as_str(), |caps: &regex::Captures| {
                        answers_by_template_key
                            .get(&caps[1])
                            .copied()
                            .unwrap_or_default()
                    })
                    .into_owned();

                let username_replaced_title =
                    answer_replaced_title.replace("$username", author_name);

                Ok(AnswerTitle::new(Some(username_replaced_title.try_into()?)))
            }
            None => Ok(AnswerTitle::new(None)),
        }
    }
}

fn question_placeholder_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\{\{question\.([A-Za-z0-9_-]+)\}\}").unwrap())
}

#[cfg(test)]
mod tests {
    use types::non_empty_string::NonEmptyString;
    use uuid::Uuid;

    use super::*;
    use crate::form::answer::models::FormAnswerContentId;
    use crate::form::{
        answer::models::FormAnswerContent,
        question::models::{QuestionId, QuestionType},
    };
    use crate::user::models::User;

    fn question_id(seed: &str) -> QuestionId {
        Uuid::parse_str(seed).unwrap().into()
    }

    #[test]
    fn test_embedded_answer_title() {
        let first_question_id = question_id("00000000-0000-7000-8000-000000000001");
        let second_question_id = question_id("00000000-0000-7000-8000-000000000002");
        let third_question_id = question_id("00000000-0000-7000-8000-000000000003");

        let default_answer_title = DefaultAnswerTitle::new(Some(
            NonEmptyString::try_new(
                "Answer to {{question.first}}, {{question.second}}, {{question.third}} by $username($username)"
                    .to_string(),
            )
            .unwrap(),
        ));
        let questions = vec![
            Question::from_raw_parts(
                first_question_id,
                "first".to_string().try_into().unwrap(),
                0,
                "First".to_string().try_into().unwrap(),
                None,
                QuestionType::Text,
                None,
                true,
            )
            .unwrap(),
            Question::from_raw_parts(
                second_question_id,
                "second".to_string().try_into().unwrap(),
                1,
                "Second".to_string().try_into().unwrap(),
                None,
                QuestionType::Text,
                None,
                true,
            )
            .unwrap(),
            Question::from_raw_parts(
                third_question_id,
                "third".to_string().try_into().unwrap(),
                2,
                "Third".to_string().try_into().unwrap(),
                None,
                QuestionType::Text,
                None,
                true,
            )
            .unwrap(),
        ];
        let answers = PostedAnswerContents::try_new(
            questions.as_slice(),
            vec![
                FormAnswerContent {
                    id: FormAnswerContentId::new(),
                    question_id: first_question_id,
                    answer: "Answer1".to_string(),
                },
                FormAnswerContent {
                    id: FormAnswerContentId::new(),
                    question_id: second_question_id,
                    answer: "Answer2".to_string(),
                },
                FormAnswerContent {
                    id: FormAnswerContentId::new(),
                    question_id: third_question_id,
                    answer: "Answer3".to_string(),
                },
            ],
        )
        .unwrap();

        let actor = User::ActiveUser(crate::user::models::ActiveUser::new(
            "respondent_name".to_string(),
            Uuid::nil().into(),
            Default::default(),
        ));

        let result = DefaultAnswerTitleDomainService::<
            crate::repository::form::active_form_repository::MockActiveFormRepository,
        >::to_answer_title_from_questions(
            default_answer_title,
            questions.as_slice(),
            &answers,
            match &actor {
                User::ActiveUser(actor) => actor.name(),
                User::TemporaryUser(actor) => actor.name(),
                User::Anonymous => unreachable!(),
            },
        )
        .unwrap();

        assert_eq!(
            result.into_inner().unwrap().into_inner(),
            "Answer to Answer1, Answer2, Answer3 by respondent_name(respondent_name)"
        );
    }
}
