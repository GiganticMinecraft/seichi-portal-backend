use crate::{
    database::components::{DatabaseComponents, FormAnswerDatabase},
    repository::Repository,
};
use async_trait::async_trait;
use domain::form::answer::service::AnswerEntryAuthorizationContext;
use domain::types::authorization_guard_with_context::{
    AuthorizationGuardWithContext, Create, Read,
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
use futures::{stream, StreamExt};
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
    async fn get_answers(&self, answer_id: AnswerId) -> Result<Option<AnswerEntry>, Error> {
        self.client
            .form_answer()
            .get_answers(answer_id)
            .await?
            .map(|posted_answers_dto| posted_answers_dto.try_into())
            .transpose()
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
    async fn get_answers_by_form_id(&self, form_id: FormId) -> Result<Vec<AnswerEntry>, Error> {
        self.client
            .form_answer()
            .get_answers_by_form_id(form_id)
            .await
            .map(|answers| {
                answers
                    .into_iter()
                    .map(|posted_answers_dto| posted_answers_dto.try_into())
                    .collect::<Result<Vec<AnswerEntry>, _>>()
            })?
            .map_err(Into::into)
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
    async fn update_answer_meta(
        &self,
        answer_id: AnswerId,
        title: Option<String>,
    ) -> Result<(), Error> {
        self.client
            .form_answer()
            .update_answer_meta(answer_id, title)
            .await
            .map_err(Into::into)
    }
}
