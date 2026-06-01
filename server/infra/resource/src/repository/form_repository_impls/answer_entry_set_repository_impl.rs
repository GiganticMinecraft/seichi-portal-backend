use async_trait::async_trait;
use domain::{
    form::{
        answer::models::AnswerEntry, answer_entry_set::models::AnswerEntrySet, models::ActiveForm,
    },
    repository::form::answer_entry_set_repository::AnswerEntrySetRepository,
    types::authorization_guard::{Allowed, Read, Update},
};
use errors::Error;

use crate::{
    database::{
        components::{DatabaseComponents, FormAnswerDatabase, FormDatabase},
        connection::DatabaseTransaction,
    },
    repository::Repository,
};

#[async_trait]
impl<Client> AnswerEntrySetRepository for Repository<Client>
where
    Client: DatabaseComponents<TransactionAcrossComponents = DatabaseTransaction> + 'static,
{
    #[tracing::instrument(skip(self))]
    async fn get_read(
        &self,
        form: &Allowed<ActiveForm, Read>,
    ) -> Result<Option<Allowed<AnswerEntrySet, Read>>, Error> {
        let record = self.client.form().get_answer_entry_set(*form.id()).await?;

        record
            .map(|set| form.authorize_read(set).map_err(Into::into))
            .transpose()
    }

    #[tracing::instrument(skip(self, form))]
    async fn get_update(
        &self,
        form: &Allowed<ActiveForm, Update>,
    ) -> Result<Option<Allowed<AnswerEntrySet, Update>>, Error> {
        let record = self.client.form().get_answer_entry_set(*form.id()).await?;

        record
            .map(|set| form.authorize_update(set).map_err(Into::into))
            .transpose()
    }

    #[tracing::instrument(skip(self, forms))]
    async fn list_read_by_forms(
        &self,
        forms: &[Allowed<ActiveForm, Read>],
    ) -> Result<Vec<Allowed<AnswerEntrySet, Read>>, Error> {
        let mut sets = Vec::with_capacity(forms.len());

        for form in forms {
            if let Some(set) = self.client.form().get_answer_entry_set(*form.id()).await? {
                sets.push(form.authorize_read(set)?);
            }
        }

        Ok(sets)
    }

    #[tracing::instrument(skip(self, answer_entry))]
    async fn add_entry(
        &self,
        answer_entry_set: &Allowed<AnswerEntrySet, Read>,
        answer_entry: &AnswerEntry,
    ) -> Result<(), Error> {
        self.client
            .form_answer()
            .post_answer(answer_entry, *answer_entry_set.value().form_id())
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self, answer_entry))]
    async fn update_entry(
        &self,
        answer_entry_set: &Allowed<AnswerEntrySet, Update>,
        answer_entry: &Allowed<AnswerEntry, Update>,
    ) -> Result<(), Error> {
        self.client
            .form_answer()
            .update_answer_entry(answer_entry.value(), *answer_entry_set.value().form_id())
            .await?;
        Ok(())
    }

    #[tracing::instrument(skip(self))]
    async fn size_entries(&self) -> Result<u32, Error> {
        self.client.form_answer().size().await.map_err(Into::into)
    }
}
