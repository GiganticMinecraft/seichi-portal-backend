use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::models::{
        AnswerId, Comment, Form, FormDescription, FormId, FormQuestionUpdateSchema, FormTitle,
        FormUpdateTargets, OffsetAndLimit, PostedAnswers, PostedAnswersSchema, Question,
        SimpleForm,
    },
    user::models::User,
};

#[automock]
#[async_trait]
pub trait FormRepository: Send + Sync + 'static {
    async fn create(
        &self,
        title: FormTitle,
        description: FormDescription,
        user: User,
    ) -> Result<FormId, Error>;
    async fn list(&self, offset_and_limit: OffsetAndLimit) -> Result<Vec<SimpleForm>, Error>;
    async fn get(&self, id: FormId) -> Result<Form, Error>;
    async fn delete(&self, id: FormId) -> Result<(), Error>;
    async fn update(
        &self,
        form_id: FormId,
        form_update_targets: FormUpdateTargets,
    ) -> Result<(), Error>;
    async fn post_answer(&self, user: &User, answers: &PostedAnswersSchema) -> Result<(), Error>;
    async fn get_answers(&self, answer_id: AnswerId) -> Result<Option<PostedAnswers>, Error>;
    async fn get_all_answers(&self) -> Result<Vec<PostedAnswers>, Error>;
    async fn create_questions(&self, questions: &FormQuestionUpdateSchema) -> Result<(), Error>;
    async fn put_questions(&self, questions: &FormQuestionUpdateSchema) -> Result<(), Error>;
    async fn get_questions(&self, form_id: FormId) -> Result<Vec<Question>, Error>;
    async fn has_permission(&self, answer_id: AnswerId, user: &User) -> Result<bool, Error>;
    async fn post_comment(&self, comment: &Comment) -> Result<(), Error>;
}
