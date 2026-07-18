use errors::Error;
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::form::{
    answer::{AnswerTitle, PostedAnswerContents},
    models::{DefaultAnswerTitle, FormTitle, Question},
};

pub struct DefaultAnswerTitleDomainService;

impl DefaultAnswerTitleDomainService {
    pub fn to_answer_title_from_questions(
        default_answer_title: DefaultAnswerTitle,
        form_title: &FormTitle,
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

                let replaced_title = template_placeholder_regex()
                    .replace_all(
                        default_answer_title.as_str(),
                        |caps: &regex::Captures| match &caps[1] {
                            "form_name" => form_title.as_str(),
                            "username" => author_name,
                            template_key => answers_by_template_key
                                .get(template_key)
                                .copied()
                                .unwrap_or_default(),
                        },
                    )
                    .into_owned();

                Ok(AnswerTitle::new(Some(replaced_title.try_into()?)))
            }
            None => Ok(AnswerTitle::new(None)),
        }
    }
}

fn template_placeholder_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\$([A-Za-z0-9_-]+)").unwrap())
}

#[cfg(test)]
mod tests {
    use types::non_empty_string::NonEmptyString;
    use uuid::Uuid;

    use super::*;
    use crate::auth::Actor;
    use crate::form::answer::FormAnswerContentId;
    use crate::form::{
        answer::FormAnswerContent,
        question::{QuestionId, QuestionType},
    };
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
                "Answer to $first, $second, $third by $username($username)".to_string(),
            )
            .unwrap(),
        ));
        let questions = unsafe {
            vec![
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
            ]
        };
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

        let actor = Actor::AccountUser(crate::account::models::AccountUser::new(
            "respondent_name".to_string(),
            Uuid::nil().into(),
            Default::default(),
        ));

        let result = DefaultAnswerTitleDomainService::to_answer_title_from_questions(
            default_answer_title,
            &FormTitle::new("Form".to_string().try_into().unwrap()),
            questions.as_slice(),
            &answers,
            match &actor {
                Actor::AccountUser(actor) => actor.name(),
                Actor::TemporaryAnswerAuthor(actor) => actor.name(),
                Actor::Anonymous => unreachable!(),
                Actor::System => unreachable!(),
            },
        )
        .unwrap();

        assert_eq!(
            result.into_inner().unwrap().into_inner(),
            "Answer to Answer1, Answer2, Answer3 by respondent_name(respondent_name)"
        );
    }

    fn title_from(title: &str, questions: &[Question], answers: &PostedAnswerContents) -> String {
        DefaultAnswerTitleDomainService::to_answer_title_from_questions(
            DefaultAnswerTitle::new(Some(title.to_string().try_into().unwrap())),
            &FormTitle::new("Form".to_string().try_into().unwrap()),
            questions,
            answers,
            "respondent",
        )
        .unwrap()
        .into_inner()
        .unwrap()
        .into_inner()
    }

    fn question(id: QuestionId, template_key: &str, position: u16) -> Question {
        unsafe {
            Question::from_raw_parts(
                id,
                template_key.parse().unwrap(),
                position,
                template_key.to_string().try_into().unwrap(),
                None,
                QuestionType::Text,
                None,
                false,
            )
            .unwrap()
        }
    }

    #[test]
    fn replaces_consecutive_placeholders_in_one_pass() {
        let first_id = question_id("00000000-0000-7000-8000-000000000011");
        let second_id = question_id("00000000-0000-7000-8000-000000000012");
        let questions = vec![
            question(first_id, "first", 0),
            question(second_id, "second", 1),
        ];
        let answers = PostedAnswerContents::try_new(
            &questions,
            vec![
                FormAnswerContent {
                    id: FormAnswerContentId::new(),
                    question_id: first_id,
                    answer: "$username".to_string(),
                },
                FormAnswerContent {
                    id: FormAnswerContentId::new(),
                    question_id: second_id,
                    answer: "second answer".to_string(),
                },
            ],
        )
        .unwrap();

        assert_eq!(
            title_from("$first$second", &questions, &answers),
            "$usernamesecond answer"
        );
    }

    #[test]
    fn replaces_reserved_and_question_placeholders_once_in_the_same_template() {
        let question_id = question_id("00000000-0000-7000-8000-000000000013");
        let questions = vec![question(question_id, "body", 0)];
        let answers = PostedAnswerContents::try_new(
            &questions,
            vec![FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id,
                answer: "$form_name from $username".to_string(),
            }],
        )
        .unwrap();
        let form_title = FormTitle::new(
            "Support for $username and $body"
                .to_string()
                .try_into()
                .unwrap(),
        );

        let title = DefaultAnswerTitleDomainService::to_answer_title_from_questions(
            DefaultAnswerTitle::new(Some(
                "$form_name by $username: $body"
                    .to_string()
                    .try_into()
                    .unwrap(),
            )),
            &form_title,
            &questions,
            &answers,
            "respondent",
        )
        .unwrap()
        .into_inner()
        .unwrap()
        .into_inner();

        assert_eq!(
            title,
            "Support for $username and $body by respondent: $form_name from $username"
        );
    }

    #[test]
    fn leaves_legacy_question_placeholders_literal() {
        let question_id = question_id("00000000-0000-7000-8000-000000000021");
        let questions = vec![question(question_id, "body", 0)];
        let answers = PostedAnswerContents::try_new(
            &questions,
            vec![FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id,
                answer: "answer".to_string(),
            }],
        )
        .unwrap();

        assert_eq!(
            title_from("{{question.body}}", &questions, &answers),
            "{{question.body}}"
        );
    }

    #[test]
    fn replaces_unknown_and_unanswered_placeholders_with_empty_strings() {
        let answered_id = question_id("00000000-0000-7000-8000-000000000031");
        let unanswered_id = question_id("00000000-0000-7000-8000-000000000032");
        let questions = vec![
            question(answered_id, "answered", 0),
            question(unanswered_id, "unanswered", 1),
        ];
        let answers = PostedAnswerContents::try_new(
            &questions,
            vec![FormAnswerContent {
                id: FormAnswerContentId::new(),
                question_id: answered_id,
                answer: "answer".to_string(),
            }],
        )
        .unwrap();

        assert_eq!(
            title_from("$answered/$unanswered/$unknown", &questions, &answers),
            "answer//"
        );
    }
}
