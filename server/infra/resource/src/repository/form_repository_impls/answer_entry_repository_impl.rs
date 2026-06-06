use std::collections::HashMap;

use async_trait::async_trait;
use domain::{
    form::{
        answer::models::{AnswerEntry, AnswerId},
        models::ActiveForm,
    },
    repository::form::answer_entry_repository::AnswerEntryRepository,
    types::authorization_guard::{Allowed, Create, Read, Update},
};
use errors::Error;
use uuid::Uuid;

use crate::{
    database::{
        components::{DatabaseComponents, FormAnswerDatabase, FormDatabase},
        connection::DatabaseTransaction,
    },
    repository::Repository,
};

#[async_trait]
impl<Client> AnswerEntryRepository for Repository<Client>
where
    Client: DatabaseComponents<TransactionAcrossComponents = DatabaseTransaction> + 'static,
{
    #[tracing::instrument(skip(self, form))]
    async fn get(
        &self,
        form: &Allowed<ActiveForm, Read>,
        answer_id: AnswerId,
    ) -> Result<Option<Allowed<AnswerEntry, Read>>, Error> {
        self.client
            .form_answer()
            .get_answers(answer_id)
            .await?
            .map(TryInto::<AnswerEntry>::try_into)
            .transpose()?
            .filter(|entry| entry.form_id() == form.id())
            .map(|entry| form.read_entry(entry))
            .transpose()
            .map_err(Into::into)
    }

    #[tracing::instrument(skip(self, form))]
    async fn list_by_form(
        &self,
        form: &Allowed<ActiveForm, Read>,
    ) -> Result<Vec<Allowed<AnswerEntry, Read>>, Error> {
        let entries = self.client.form().list_answer_entries(*form.id()).await?;

        Ok(form.readable_entries(entries))
    }

    #[tracing::instrument(skip(self, forms))]
    async fn list_all(
        &self,
        forms: &[Allowed<ActiveForm, Read>],
    ) -> Result<Vec<Allowed<AnswerEntry, Read>>, Error> {
        if forms.is_empty() {
            return Ok(Vec::new());
        }

        let mut entries_by_form = self
            .client
            .form()
            .list_all_answer_entries()
            .await?
            .into_iter()
            .fold(
                HashMap::<Uuid, Vec<AnswerEntry>>::new(),
                |mut acc, entry| {
                    acc.entry(entry.form_id().into_inner())
                        .or_default()
                        .push(entry);
                    acc
                },
            );

        Ok(forms
            .iter()
            .flat_map(|form| {
                form.readable_entries(
                    entries_by_form
                        .remove(&form.id().into_inner())
                        .unwrap_or_default(),
                )
            })
            .collect())
    }

    #[tracing::instrument(skip(self, _form, answer_entry))]
    async fn post(
        &self,
        _form: &Allowed<ActiveForm, Read>,
        answer_entry: &Allowed<AnswerEntry, Create>,
    ) -> Result<(), Error> {
        self.client
            .form_answer()
            .post_answer(answer_entry.value(), *answer_entry.value().form_id())
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self, _form, answer_entry))]
    async fn update(
        &self,
        _form: &Allowed<ActiveForm, Update>,
        answer_entry: &Allowed<AnswerEntry, Update>,
    ) -> Result<(), Error> {
        self.client
            .form_answer()
            .update_answer_entry(answer_entry.value(), *answer_entry.value().form_id())
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn size(&self) -> Result<u32, Error> {
        self.client.form_answer().size().await.map_err(Into::into)
    }
}
