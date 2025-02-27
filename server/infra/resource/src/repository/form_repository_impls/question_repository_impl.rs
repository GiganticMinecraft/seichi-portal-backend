use async_trait::async_trait;
use domain::{
    form::{models::FormId, question::models::Question},
    repository::form::question_repository::QuestionRepository,
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard_with_context::{Create, Read, Update},
    },
    user::models::User,
};
use errors::Error;
use itertools::Itertools;

use crate::{
    database::components::{DatabaseComponents, FormQuestionDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> QuestionRepository for Repository<Client> {
    #[tracing::instrument(skip(self))]
    async fn create_questions(
        &self,
        actor: &User,
        form_id: FormId,
        questions: Vec<AuthorizationGuard<Question, Create>>,
    ) -> Result<(), Error> {
        let questions = questions
            .into_iter()
            .map(|guard| guard.try_into_create(actor, |form| form))
            .collect::<Result<Vec<_>, _>>()?;

        self.client
            .form_question()
            .create_questions(form_id, questions)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn put_questions(
        &self,
        actor: &User,
        form_id: FormId,
        questions: Vec<AuthorizationGuard<Question, Update>>,
    ) -> Result<(), Error> {
        let questions = questions
            .into_iter()
            .map(|guard| guard.try_into_update(actor, |question| question))
            .collect::<Result<Vec<_>, _>>()?;

        self.client
            .form_question()
            .put_questions(form_id, questions)
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn get_questions(
        &self,
        form_id: FormId,
    ) -> Result<Vec<AuthorizationGuard<Question, Read>>, Error> {
        self.client
            .form_question()
            .get_questions(form_id)
            .await?
            .into_iter()
            .map(TryInto::<Question>::try_into)
            .map_ok(Into::into)
            .collect::<Result<Vec<_>, _>>()
    }
}
