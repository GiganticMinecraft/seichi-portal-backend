use crate::{
    database::components::{DatabaseComponents, FormAnswerDatabase},
    repository::Repository,
};
use async_trait::async_trait;
use domain::form::answer::service::AnswerEntryAuthorizationContext;
use domain::types::authorization_guard_with_context::{
    AuthorizationGuardWithContext, Create, Read, Update,
};
use domain::user::models::User;
use domain::{
    form::{
        answer::models::{AnswerEntry, AnswerId, FormAnswerContent},
        models::FormId,
    },
    repository::form::answer_repository::AnswerRepository,
};
use errors::Error;
use itertools::Itertools;

#[async_trait]
impl<Client: DatabaseComponents + 'static> AnswerRepository for Repository<Client> {
    #[tracing::instrument(skip(self))]
    async fn post_answer<'a>(
        &self,
        context: &'a AnswerEntryAuthorizationContext,
        answer: AuthorizationGuardWithContext<AnswerEntry, Create, AnswerEntryAuthorizationContext>,
        content: Vec<FormAnswerContent>,
        actor: &User,
    ) -> Result<(), Error> {
        answer
            .try_create(
                actor,
                |entry| self.client.form_answer().post_answer(entry, content),
                context,
            )?
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn get_answer(
        &self,
        answer_id: AnswerId,
    ) -> Result<
        Option<AuthorizationGuardWithContext<AnswerEntry, Read, AnswerEntryAuthorizationContext>>,
        Error,
    > {
        Ok(self
            .client
            .form_answer()
            .get_answers(answer_id)
            .await?
            .map(TryInto::<AnswerEntry>::try_into)
            .transpose()?
            .map(|entry| AuthorizationGuardWithContext::new(entry).into_read()))
    }

    #[tracing::instrument(skip(self))]
    async fn get_answer_contents(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<FormAnswerContent>, Error> {
        self.client
            .form_answer()
            .get_answer_contents(answer_id)
            .await
            .map(|answer_contents| {
                answer_contents
                    .into_iter()
                    .map(|answer_content_dto| answer_content_dto.try_into())
                    .collect::<Result<Vec<FormAnswerContent>, _>>()
            })?
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn get_answers_by_form_id(
        &self,
        form_id: FormId,
    ) -> Result<
        Vec<AuthorizationGuardWithContext<AnswerEntry, Read, AnswerEntryAuthorizationContext>>,
        Error,
    > {
        Ok(self
            .client
            .form_answer()
            .get_answers_by_form_id(form_id)
            .await
            .map(|answers| {
                answers
                    .into_iter()
                    .map(|posted_answers_dto| posted_answers_dto.try_into())
                    .collect::<Result<Vec<AnswerEntry>, _>>()
            })??
            .into_iter()
            .map(|entry| AuthorizationGuardWithContext::new(entry).into_read())
            .collect_vec())
    }

    #[tracing::instrument(skip(self))]
    async fn get_all_answers(
        &self,
    ) -> Result<
        Vec<AuthorizationGuardWithContext<AnswerEntry, Read, AnswerEntryAuthorizationContext>>,
        Error,
    > {
        self.client
            .form_answer()
            .get_all_answers()
            .await?
            .into_iter()
            .map(TryInto::<AnswerEntry>::try_into)
            .map_ok(|entry| AuthorizationGuardWithContext::new(entry).into_read())
            .collect::<Result<Vec<_>, _>>()
    }

    #[tracing::instrument(skip(self))]
    async fn update_answer_entry(
        &self,
        actor: &User,
        context: &AnswerEntryAuthorizationContext,
        answer_entry: AuthorizationGuardWithContext<
            AnswerEntry,
            Update,
            AnswerEntryAuthorizationContext,
        >,
    ) -> Result<(), Error> {
        answer_entry
            .try_update(
                actor,
                |entry| self.client.form_answer().update_answer_entry(entry),
                context,
            )?
            .await
            .map_err(Into::into)
    }
}
