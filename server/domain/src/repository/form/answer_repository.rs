use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::form::answer::service::AnswerEntryAuthorizationContext;
use crate::form::{
    answer::models::{AnswerEntry, AnswerId, FormAnswerContent},
    models::FormId,
};
use crate::types::authorization_guard_with_context::{AuthorizationGuardWithContext, Create};
use crate::user::models::User;

#[automock]
#[async_trait]
pub trait AnswerRepository: Send + Sync + 'static {
    async fn post_answer<'a>(
        &self,
        answer: AuthorizationGuardWithContext<
            AnswerEntry,
            Create,
            AnswerEntryAuthorizationContext<'a>,
        >,
        content: Vec<FormAnswerContent>,
        actor: &User,
    ) -> Result<(), Error>;
    async fn get_answers(&self, answer_id: AnswerId) -> Result<Option<AnswerEntry>, Error>;
    async fn get_answer_contents(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<FormAnswerContent>, Error>;
    async fn get_answers_by_form_id(&self, form_id: FormId) -> Result<Vec<AnswerEntry>, Error>;
    async fn get_all_answers(&self) -> Result<Vec<AnswerEntry>, Error>;
    async fn update_answer_meta(
        &self,
        answer_id: AnswerId,
        title: Option<String>,
    ) -> Result<(), Error>;
}
