use errors::{Error, domain::DomainError};
use regex::Regex;

use crate::{
    form::{
        answer::{
            models::{AnswerTitle, FormAnswerContent},
            settings::models::DefaultAnswerTitle,
        },
        models::FormId,
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
    fn generate_embedded_answer_title(
        default_answer_title: DefaultAnswerTitle,
        answers: &[FormAnswerContent],
        actor: &User,
    ) -> Result<AnswerTitle, Error> {
        match default_answer_title.into_inner() {
            Some(default_answer_title) => {
                let default_answer_title = default_answer_title.to_string();
                let regex = Regex::new(r"\$\d+").unwrap();

                let answer_replaced_title: String = regex
                    .find_iter(default_answer_title.to_owned().as_str())
                    .fold(default_answer_title, |replaced_title, question_id| {
                        let answer_opt = answers.iter().find(|answer| {
                            answer.question_id.to_string() == question_id.as_str().replace('$', "")
                        });
                        replaced_title.replace(
                            question_id.as_str(),
                            &answer_opt
                                .map(|answer| answer.answer.to_owned().to_string())
                                .unwrap_or_default(),
                        )
                    });

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
        answers: &[FormAnswerContent],
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

        Self::generate_embedded_answer_title(default_answer_title, answers, actor)
    }
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
            NonEmptyString::try_new(format!(
                "Answer to ${}, ${}, ${} by $username($username)",
                first_question_id, second_question_id, third_question_id
            ))
            .unwrap(),
        ));
        let answers = vec![
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
        ];

        let actor = User {
            name: "respondent_name".to_string(),
            id: Default::default(),
            role: Default::default(),
        };

        let result = DefaultAnswerTitleDomainService::<
            MockFormRepository,
            MockQuestionRepository,
            MockAnswerRepository,
        >::generate_embedded_answer_title(
            default_answer_title, answers.as_slice(), &actor
        )
        .unwrap();

        assert_eq!(
            result.into_inner().unwrap().into_inner(),
            "Answer to Answer1, Answer2, Answer3 by respondent_name(respondent_name)"
        );
    }
}
