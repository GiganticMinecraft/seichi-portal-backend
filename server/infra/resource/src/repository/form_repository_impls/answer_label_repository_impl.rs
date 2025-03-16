use async_trait::async_trait;
use domain::{
    form::answer::models::{AnswerId, AnswerLabel, AnswerLabelId},
    repository::form::answer_label_repository::AnswerLabelRepository,
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard_with_context::{Create, Delete, Read, Update},
    },
    user::models::User,
};
use errors::Error;
use itertools::Itertools;

use crate::{
    database::components::{DatabaseComponents, FormAnswerLabelDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client: DatabaseComponents + 'static> AnswerLabelRepository for Repository<Client> {
    #[tracing::instrument(skip(self))]
    async fn create_label_for_answers(
        &self,
        actor: &User,
        label: AuthorizationGuard<AnswerLabel, Create>,
    ) -> Result<(), Error> {
        label
            .try_create(actor, |label| {
                self.client
                    .form_answer_label()
                    .create_label_for_answers(label.name().to_owned())
            })?
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn get_labels_for_answers(
        &self,
    ) -> Result<Vec<AuthorizationGuard<AnswerLabel, Read>>, Error> {
        self.client
            .form_answer_label()
            .get_labels_for_answers()
            .await?
            .into_iter()
            .map(TryInto::<AnswerLabel>::try_into)
            .map_ok(Into::<AuthorizationGuard<AnswerLabel, Read>>::into)
            .collect::<Result<Vec<_>, _>>()
    }

    #[tracing::instrument(skip(self))]
    async fn get_label_for_answers(
        &self,
        label_id: AnswerLabelId,
    ) -> Result<Option<AuthorizationGuard<AnswerLabel, Read>>, Error> {
        Ok(self
            .client
            .form_answer_label()
            .get_label_for_answers(label_id)
            .await?
            .map(TryInto::<AnswerLabel>::try_into)
            .transpose()?
            .map(Into::<AuthorizationGuard<AnswerLabel, Read>>::into))
    }

    #[tracing::instrument(skip(self))]
    async fn get_labels_for_answers_by_label_ids(
        &self,
        label_ids: Vec<AnswerLabelId>,
    ) -> Result<Vec<AuthorizationGuard<AnswerLabel, Read>>, Error> {
        self.client
            .form_answer_label()
            .get_labels_for_answers_by_label_ids(label_ids)
            .await?
            .into_iter()
            .map(TryInto::<AnswerLabel>::try_into)
            .map_ok(Into::<AuthorizationGuard<AnswerLabel, Read>>::into)
            .collect::<Result<Vec<_>, _>>()
    }

    #[tracing::instrument(skip(self))]
    async fn get_labels_for_answers_by_answer_id(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<AuthorizationGuard<AnswerLabel, Read>>, Error> {
        self.client
            .form_answer_label()
            .get_labels_for_answers_by_answer_id(answer_id)
            .await?
            .into_iter()
            .map(TryInto::<AnswerLabel>::try_into)
            .map_ok(Into::<AuthorizationGuard<AnswerLabel, Read>>::into)
            .collect::<Result<Vec<_>, _>>()
    }

    #[tracing::instrument(skip(self))]
    async fn delete_label_for_answers(
        &self,
        actor: &User,
        label: AuthorizationGuard<AnswerLabel, Delete>,
    ) -> Result<(), Error> {
        label
            .try_delete(actor, |label| {
                self.client
                    .form_answer_label()
                    .delete_label_for_answers(*label.id())
            })?
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn edit_label_for_answers(
        &self,
        actor: &User,
        label: AuthorizationGuard<AnswerLabel, Update>,
    ) -> Result<(), Error> {
        label
            .try_update(actor, |label| {
                self.client
                    .form_answer_label()
                    .edit_label_for_answers(label)
            })?
            .await
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self))]
    async fn replace_answer_labels(
        &self,
        actor: &User,
        answer_id: AnswerId,
        labels: Vec<AuthorizationGuard<AnswerLabel, Update>>,
    ) -> Result<(), Error> {
        let label_ids = labels
            .into_iter()
            .map(|guard| guard.try_into_update(actor, |label| *label.id()))
            .collect::<Result<Vec<_>, _>>()?;

        self.client
            .form_answer_label()
            .replace_answer_labels(answer_id, label_ids)
            .await
            .map_err(Into::into)
    }
}
