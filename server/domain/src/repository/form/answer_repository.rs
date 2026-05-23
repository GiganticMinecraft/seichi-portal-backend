use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::{
        answer::{
            models::{AnswerEntry, AnswerId},
            service::{AnswerEntryActor, AnswerEntryAuthorizationContext},
        },
        models::FormId,
    },
    types::authorization_guard_with_context::{
        AuthorizationGuardWithContext, Create, Read, Update,
    },
};

#[automock]
#[async_trait]
pub trait AnswerRepository: Send + Sync + 'static {
    async fn post_answer(
        &self,
        context: &AnswerEntryAuthorizationContext,
        answer: AuthorizationGuardWithContext<
            AnswerEntry,
            Create,
            AnswerEntryAuthorizationContext,
            AnswerEntryActor,
        >,
        actor: &AnswerEntryActor,
    ) -> Result<(), Error>;
    async fn get_answer(
        &self,
        answer_id: AnswerId,
    ) -> Result<
        Option<
            AuthorizationGuardWithContext<
                AnswerEntry,
                Read,
                AnswerEntryAuthorizationContext,
                AnswerEntryActor,
            >,
        >,
        Error,
    >;
    async fn get_answers_by_form_id(
        &self,
        form_id: FormId,
    ) -> Result<
        Vec<
            AuthorizationGuardWithContext<
                AnswerEntry,
                Read,
                AnswerEntryAuthorizationContext,
                AnswerEntryActor,
            >,
        >,
        Error,
    >;
    async fn get_all_answers(
        &self,
    ) -> Result<
        Vec<
            AuthorizationGuardWithContext<
                AnswerEntry,
                Read,
                AnswerEntryAuthorizationContext,
                AnswerEntryActor,
            >,
        >,
        Error,
    >;
    async fn update_answer_entry(
        &self,
        actor: &AnswerEntryActor,
        context: &AnswerEntryAuthorizationContext,
        answer_entry: AuthorizationGuardWithContext<
            AnswerEntry,
            Update,
            AnswerEntryAuthorizationContext,
            AnswerEntryActor,
        >,
    ) -> Result<(), Error>;
    async fn size(&self) -> Result<u32, Error>;
}
