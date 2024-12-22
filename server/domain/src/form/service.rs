use errors::{domain::DomainError, Error};
use regex::Regex;

use crate::{
    form::{
        answer::models::{AnswerId, AnswerTitle, FormAnswerContent},
        models::{DefaultAnswerTitle, FormId},
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
    fn embedded_answer_title(
        default_answer_title: DefaultAnswerTitle,
        answers: Vec<FormAnswerContent>,
    ) -> Result<AnswerTitle, Error> {
        match default_answer_title.into_inner() {
            Some(default_answer_title) => {
                let default_answer_title = default_answer_title.to_string();
                let regex = Regex::new(r"\$\d+").unwrap();

                let answer_title: String = regex
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

                Ok(AnswerTitle::new(Some(answer_title.try_into()?)))
            }
            None => Ok(AnswerTitle::new(None)),
        }
    }

    pub async fn to_answer_title(
        &self,
        actor: &User,
        form_id: FormId,
        answer_id: AnswerId,
    ) -> Result<AnswerTitle, Error> {
        let form = self
            .form_repo
            .get(form_id)
            .await?
            .ok_or(DomainError::NotFound)?
            .try_into_read(actor)?;

        let default_answer_title = form.settings().default_answer_title().to_owned();

        let answers = self.answer_repo.get_answer_contents(answer_id).await?;

        Self::embedded_answer_title(default_answer_title, answers)
    }
}
