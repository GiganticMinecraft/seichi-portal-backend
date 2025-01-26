use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::{
        answer::{
            models::{AnswerEntry, AnswerId, FormAnswerContent},
            service::AnswerEntryAuthorizationContext,
        },
        models::FormId,
    },
    types::authorization_guard_with_context::{
        AuthorizationGuardWithContext, Create, Read, Update,
    },
    user::models::User,
};

#[automock]
#[async_trait]
pub trait AnswerRepository: Send + Sync + 'static {
    async fn post_answer<'a>(
        &self,
        context: &'a AnswerEntryAuthorizationContext,
        answer: AuthorizationGuardWithContext<AnswerEntry, Create, AnswerEntryAuthorizationContext>,
        content: Vec<FormAnswerContent>,
        actor: &User,
    ) -> Result<(), Error>;
    async fn get_answer(
        &self,
        answer_id: AnswerId,
    ) -> Result<
        Option<AuthorizationGuardWithContext<AnswerEntry, Read, AnswerEntryAuthorizationContext>>,
        Error,
    >;
    async fn get_answer_contents(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<FormAnswerContent>, Error>;
    async fn get_answers_by_form_id(
        &self,
        form_id: FormId,
    ) -> Result<
        Vec<AuthorizationGuardWithContext<AnswerEntry, Read, AnswerEntryAuthorizationContext>>,
        Error,
    >;
    async fn get_all_answers(
        &self,
    ) -> Result<
        Vec<AuthorizationGuardWithContext<AnswerEntry, Read, AnswerEntryAuthorizationContext>>,
        Error,
    >;
    async fn update_answer_entry(
        &self,
        actor: &User,
        context: &AnswerEntryAuthorizationContext,
        answer_entry: AuthorizationGuardWithContext<
            AnswerEntry,
            Update,
            AnswerEntryAuthorizationContext,
        >,
    ) -> Result<(), Error>;
}
