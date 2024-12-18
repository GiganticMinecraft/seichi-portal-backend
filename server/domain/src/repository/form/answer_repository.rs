use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::{
        answer::models::{AnswerId, FormAnswer, FormAnswerContent},
        models::{DefaultAnswerTitle, FormId},
    },
    user::models::User,
};

#[automock]
#[async_trait]
pub trait AnswerRepository: Send + Sync + 'static {
    async fn post_answer(
        &self,
        user: &User,
        form_id: FormId,
        title: DefaultAnswerTitle,
        answers: Vec<FormAnswerContent>,
    ) -> Result<(), Error>;
    async fn get_answers(&self, answer_id: AnswerId) -> Result<Option<FormAnswer>, Error>;
    async fn get_answer_contents(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<FormAnswerContent>, Error>;
    async fn get_answers_by_form_id(&self, form_id: FormId) -> Result<Vec<FormAnswer>, Error>;
    async fn get_all_answers(&self) -> Result<Vec<FormAnswer>, Error>;
    async fn update_answer_meta(
        &self,
        answer_id: AnswerId,
        title: Option<String>,
    ) -> Result<(), Error>;
}
