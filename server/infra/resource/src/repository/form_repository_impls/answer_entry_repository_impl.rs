use std::collections::HashMap;

use async_trait::async_trait;
use domain::{
    form::{
        answer::{AnswerEntry, AnswerId, AnswerPagePosition},
        models::ActiveForm,
    },
    pagination::{Page, PageRequest},
    repository::form::answer_entry_repository::AnswerEntryRepository,
    types::authorization_guard::{Allowed, Create, Read, Update},
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
        request: PageRequest<AnswerPagePosition>,
    ) -> Result<Page<Allowed<AnswerEntry, Read>, AnswerPagePosition>, Error> {
        let page = self
            .client
            .form()
            .list_answer_entries(*form.id(), request)
            .await?;
        let (entries, next) = page.into_parts();

        Ok(Page::new(form.readable_entries(entries), next))
    }

    #[tracing::instrument(skip(self, forms))]
    async fn list_all(
        &self,
        forms: &[Allowed<ActiveForm, Read>],
        request: PageRequest<AnswerPagePosition>,
    ) -> Result<Page<Allowed<AnswerEntry, Read>, AnswerPagePosition>, Error> {
        if forms.is_empty() {
            return Ok(Page::new(Vec::new(), None));
        }

        let page = self.client.form().list_all_answer_entries(request).await?;
        let (entries, next) = page.into_parts();
        let forms_by_id = forms
            .iter()
            .map(|form| (form.id().into_inner(), form))
            .collect::<HashMap<_, _>>();
        let authorized_entries = entries
            .into_iter()
            .filter_map(|entry| {
                forms_by_id
                    .get(&entry.form_id().into_inner())
                    .map(|form| form.read_entry(entry))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Page::new(authorized_entries, next))
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
