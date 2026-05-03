use errors::{Error, domain::DomainError};
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::{
    form::{
        answer::{
            models::{AnswerTitle, PostedAnswerContents},
            settings::models::DefaultAnswerTitle,
        },
        models::FormId,
        question::models::Question,
    },
    repository::form::{
        answer_repository::AnswerRepository, form_repository::FormRepository,
        question_repository::QuestionRepository,
    },
    user::models::User,
};

pub struct DefaultAnswerTitleDomainService<
    'a,
    FormRepo: FormRepository,
    QuestionRepo: QuestionRepository,
    AnswerRepo: AnswerRepository,
> {
    pub form_repo: &'a FormRepo,
    pub question_repo: &'a QuestionRepo,
    pub answer_repo: &'a AnswerRepo,
}

impl<FormRepo: FormRepository, QuestionRepo: QuestionRepository, AnswerRepo: AnswerRepository>
    DefaultAnswerTitleDomainService<'_, FormRepo, QuestionRepo, AnswerRepo>
{
    pub fn to_answer_title_from_questions(
        default_answer_title: DefaultAnswerTitle,
        questions: &[Question],
        answers: &PostedAnswerContents,
        actor: &User,
    ) -> Result<AnswerTitle, Error> {
        match default_answer_title.into_inner() {
            Some(default_answer_title) => {
                let default_answer_title = default_answer_title.to_string();
                let question_template_key_by_id = questions
                    .iter()
                    .filter_map(|question| {
                        question
                            .id
                            .map(|id| (id.into_inner(), question.template_key.as_str()))
                    })
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
                    answer_replaced_title.replace("$username", actor.name.as_str());

                Ok(AnswerTitle::new(Some(username_replaced_title.try_into()?)))
            }
            None => Ok(AnswerTitle::new(None)),
        }
    }

    pub async fn to_answer_title(
        &self,
        actor: &User,
        form_id: FormId,
        answers: &PostedAnswerContents,
    ) -> Result<AnswerTitle, Error> {
        let form = self
            .form_repo
            .get(form_id)
            .await?
            .ok_or(DomainError::NotFound)?
            .try_into_read(actor)?;

        let default_answer_title = form
            .settings()
            .answer_settings()
            .default_answer_title()
            .to_owned();
        let questions = self
            .question_repo
            .get_questions(form_id)
            .await?
            .into_iter()
            .map(|question| question.try_into_read(actor))
            .collect::<Result<Vec<_>, _>>()?;

        Self::to_answer_title_from_questions(default_answer_title, &questions, answers, actor)
    }
}

fn question_placeholder_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| Regex::new(r"\{\{question\.([A-Za-z0-9_-]+)\}\}").unwrap())
}

#[cfg(test)]
mod tests {
    use types::non_empty_string::NonEmptyString;

    use super::*;
    use crate::form::answer::models::FormAnswerContentId;
    use crate::{
        form::{answer::models::FormAnswerContent, question::models::QuestionId},
        repository::form::{
            answer_repository::MockAnswerRepository, form_repository::MockFormRepository,
            question_repository::MockQuestionRepository,
        },
    };

    #[test]
    fn test_embedded_answer_title() {
        let first_question_id = QuestionId::from(0);
        let second_question_id = QuestionId::from(1);
        let third_question_id = QuestionId::from(2);

        let default_answer_title = DefaultAnswerTitle::new(Some(
            NonEmptyString::try_new(
                "Answer to {{question.first}}, {{question.second}}, {{question.third}} by $username($username)"
                    .to_string(),
            )
            .unwrap(),
        ));
        let questions = vec![
            Question::from_raw_parts(
                Some(first_question_id),
                Default::default(),
                "first".to_string(),
                0,
                "First".to_string(),
                None,
                crate::form::question::models::QuestionType::Text,
                None,
                true,
            )
            .unwrap(),
            Question::from_raw_parts(
                Some(second_question_id),
                Default::default(),
                "second".to_string(),
                1,
                "Second".to_string(),
                None,
                crate::form::question::models::QuestionType::Text,
                None,
                true,
            )
            .unwrap(),
            Question::from_raw_parts(
                Some(third_question_id),
                Default::default(),
                "third".to_string(),
                2,
                "Third".to_string(),
                None,
                crate::form::question::models::QuestionType::Text,
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

        let actor = User {
            name: "respondent_name".to_string(),
            id: Default::default(),
            role: Default::default(),
        };

        let result = DefaultAnswerTitleDomainService::<
            MockFormRepository,
            MockQuestionRepository,
            MockAnswerRepository,
        >::to_answer_title_from_questions(
            default_answer_title,
            questions.as_slice(),
            &answers,
            &actor,
        )
        .unwrap();

        assert_eq!(
            result.into_inner().unwrap().into_inner(),
            "Answer to Answer1, Answer2, Answer3 by respondent_name(respondent_name)"
        );
    }
}
