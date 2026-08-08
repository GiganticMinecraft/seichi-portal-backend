use std::collections::HashMap;

use async_trait::async_trait;
use domain::{
    account::models::UserSnapshot,
    auth::Actor,
    form::{
        answer::{
            AnswerEntry, AnswerId, AnswerPagePosition, AnswerStatus, AnswerStatusHistoryEntry,
            AnswerStatusHistoryPagePosition,
        },
        models::ActiveForm,
    },
    pagination::{Page, PageRequest},
    repository::form::answer_entry_repository::AnswerEntryRepository,
    types::authorization_guard::{Allowed, Create, Read, Update},
};
use errors::{Error, infra::InfraError};
use uuid::Uuid;

use crate::{
    database::components::{DatabaseComponents, FormAnswerDatabase, FormDatabase},
    repository::Repository,
};

#[async_trait]
impl<Client> AnswerEntryRepository for Repository<Client>
where
    Client: DatabaseComponents + 'static,
{
    #[tracing::instrument(skip_all, fields(answer_id = %answer_id))]
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

    #[tracing::instrument(skip_all)]
    async fn find_by_ids(
        &self,
        forms: &[Allowed<ActiveForm, Read>],
        answer_ids: Vec<AnswerId>,
    ) -> Result<Vec<Allowed<AnswerEntry, Read>>, Error> {
        if forms.is_empty() || answer_ids.is_empty() {
            return Ok(Vec::new());
        }

        let entries = self
            .client
            .form_answer()
            .get_answers_by_answer_ids(answer_ids)
            .await?
            .into_iter()
            .map(TryInto::<AnswerEntry>::try_into)
            .collect::<Result<Vec<_>, _>>()?;
        let forms_by_id = forms
            .iter()
            .map(|form| (form.id().into_inner(), form))
            .collect::<HashMap<_, _>>();

        Ok(entries
            .into_iter()
            .filter_map(|entry| {
                forms_by_id
                    .get(&entry.form_id().into_inner())
                    .and_then(|form| form.read_entry(entry).ok())
            })
            .collect())
    }

    #[tracing::instrument(skip_all)]
    async fn list_by_form(
        &self,
        form: &Allowed<ActiveForm, Read>,
        request: PageRequest<AnswerPagePosition>,
        status: Option<AnswerStatus>,
    ) -> Result<Page<Allowed<AnswerEntry, Read>, AnswerPagePosition>, Error> {
        let mut scan_cursor = request.after_position().copied();
        let mut authorized_entries = Vec::new();

        loop {
            let page = self
                .client
                .form()
                .list_answer_entries(
                    *form.id(),
                    PageRequest::new(scan_cursor, request.limit()),
                    status,
                )
                .await?;
            let (entries, next_raw) = page.into_parts();
            authorized_entries.extend(
                entries
                    .into_iter()
                    .filter_map(|entry| form.read_entry(entry).ok()),
            );

            if authorized_entries.len() > request.limit().value() as usize {
                return Ok(Page::from_overfetched_items(
                    authorized_entries,
                    request.limit(),
                    |entry| AnswerPagePosition::new(*entry.timestamp(), *entry.id()),
                ));
            }

            match next_raw {
                Some(_) if authorized_entries.len() == request.limit().value() as usize => {
                    let next = authorized_entries
                        .last()
                        .map(|entry| AnswerPagePosition::new(*entry.timestamp(), *entry.id()));
                    return Ok(Page::new(authorized_entries, next));
                }
                Some(next_raw) => scan_cursor = Some(next_raw),
                None => return Ok(Page::new(authorized_entries, None)),
            }
        }
    }

    #[tracing::instrument(skip_all)]
    async fn list_all(
        &self,
        forms: &[Allowed<ActiveForm, Read>],
        request: PageRequest<AnswerPagePosition>,
        status: Option<AnswerStatus>,
    ) -> Result<Page<Allowed<AnswerEntry, Read>, AnswerPagePosition>, Error> {
        if forms.is_empty() {
            return Ok(Page::new(Vec::new(), None));
        }

        let forms_by_id = forms
            .iter()
            .map(|form| (form.id().into_inner(), form))
            .collect::<HashMap<_, _>>();
        let mut scan_cursor = request.after_position().copied();
        let mut authorized_entries = Vec::new();

        loop {
            let page = self
                .client
                .form()
                .list_all_answer_entries(PageRequest::new(scan_cursor, request.limit()), status)
                .await?;
            let (entries, next_raw) = page.into_parts();
            authorized_entries.extend(entries.into_iter().filter_map(|entry| {
                forms_by_id
                    .get(&entry.form_id().into_inner())
                    .and_then(|form| form.read_entry(entry).ok())
            }));

            if authorized_entries.len() > request.limit().value() as usize {
                return Ok(Page::from_overfetched_items(
                    authorized_entries,
                    request.limit(),
                    |entry| AnswerPagePosition::new(*entry.timestamp(), *entry.id()),
                ));
            }

            match next_raw {
                Some(_) if authorized_entries.len() == request.limit().value() as usize => {
                    let next = authorized_entries
                        .last()
                        .map(|entry| AnswerPagePosition::new(*entry.timestamp(), *entry.id()));
                    return Ok(Page::new(authorized_entries, next));
                }
                Some(next_raw) => scan_cursor = Some(next_raw),
                None => return Ok(Page::new(authorized_entries, None)),
            }
        }
    }

    #[tracing::instrument(skip_all)]
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

    #[tracing::instrument(skip_all)]
    async fn update(
        &self,
        _form: &Allowed<ActiveForm, Update>,
        answer_entry: &Allowed<AnswerEntry, Update>,
    ) -> Result<(), Error> {
        self.client
            .form_answer()
            .update_answer_entry(
                answer_entry.value(),
                *answer_entry.value().form_id(),
                match answer_entry.actor() {
                    Actor::AccountUser(user) => user,
                    Actor::TemporaryAnswerAuthor(_) | Actor::Anonymous | Actor::System => {
                        return Err(InfraError::Unexpected {
                            cause: "answer update actor is not an account user".to_string(),
                        }
                        .into());
                    }
                },
            )
            .await?;
        Ok(())
    }

    async fn history(
        &self,
        answer: &Allowed<AnswerEntry, Read>,
        request: PageRequest<AnswerStatusHistoryPagePosition>,
    ) -> Result<Page<Allowed<AnswerStatusHistoryEntry, Read>, AnswerStatusHistoryPagePosition>, Error>
    {
        let page = self
            .client
            .form_answer()
            .fetch_status_history(*answer.id(), request)
            .await?;
        let (records, next) = page.into_parts();
        let items = records
            .into_iter()
            .map(|record| {
                let entry = unsafe {
                    AnswerStatusHistoryEntry::from_raw_parts(
                        Uuid::parse_str(&record.id)
                            .map_err(InfraError::from)?
                            .into(),
                        Uuid::parse_str(&record.answer_id)
                            .map_err(InfraError::from)?
                            .into(),
                        AnswerStatus::try_from(record.from_status).map_err(Error::from)?,
                        AnswerStatus::try_from(record.to_status).map_err(Error::from)?,
                        UserSnapshot::new(
                            Uuid::parse_str(&record.changed_by_id)
                                .map_err(InfraError::from)?
                                .into(),
                            record.changed_by_name,
                            record.changed_by_role.parse().map_err(InfraError::from)?,
                        ),
                        record.changed_at,
                    )
                };
                answer
                    .authorize_status_history_entry(entry)
                    .map_err(Error::from)
            })
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(Page::new(items, next))
    }

    #[tracing::instrument(skip_all)]
    async fn size(&self) -> Result<u32, Error> {
        self.client.form_answer().size().await.map_err(Into::into)
    }

    #[tracing::instrument(skip_all)]
    async fn content_size(&self) -> Result<u32, Error> {
        self.client
            .form_answer()
            .content_size()
            .await
            .map_err(Into::into)
    }
}
