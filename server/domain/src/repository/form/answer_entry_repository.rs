use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::{
        answer::{AnswerEntry, AnswerId, AnswerPagePosition},
        models::ActiveForm,
    },
    pagination::{Page, PageRequest},
    types::authorization_guard::{Allowed, Create, Read, Update},
};

#[automock]
#[async_trait]
pub trait AnswerEntryRepository: Send + Sync + 'static {
    async fn get(
        &self,
        form: &Allowed<ActiveForm, Read>,
        answer_id: AnswerId,
    ) -> Result<Option<Allowed<AnswerEntry, Read>>, Error>;
    /// 指定された ID に一致する回答のうち、`forms` に含まれる親フォームから
    /// 閲覧を認可できるものだけを返す。
    ///
    /// 指定されていない ID、親フォームが `forms` にない回答、または閲覧できない回答は
    /// 返さない。返却順は規定しない。`answer_ids` が空の場合は空のリストを返す。
    async fn find_by_ids(
        &self,
        forms: &[Allowed<ActiveForm, Read>],
        answer_ids: Vec<AnswerId>,
    ) -> Result<Vec<Allowed<AnswerEntry, Read>>, Error>;
    async fn list_by_form(
        &self,
        form: &Allowed<ActiveForm, Read>,
        request: PageRequest<AnswerPagePosition>,
    ) -> Result<Page<Allowed<AnswerEntry, Read>, AnswerPagePosition>, Error>;
    async fn list_all(
        &self,
        forms: &[Allowed<ActiveForm, Read>],
        request: PageRequest<AnswerPagePosition>,
    ) -> Result<Page<Allowed<AnswerEntry, Read>, AnswerPagePosition>, Error>;
    async fn post(
        &self,
        form: &Allowed<ActiveForm, Read>,
        answer_entry: &Allowed<AnswerEntry, Create>,
    ) -> Result<(), Error>;
    async fn update(
        &self,
        form: &Allowed<ActiveForm, Update>,
        answer_entry: &Allowed<AnswerEntry, Update>,
    ) -> Result<(), Error>;
    /// 回答 (`answers`) の件数を返す。
    async fn size(&self) -> Result<u32, Error>;
    /// 回答本文 (`real_answers`) の件数を返す。
    async fn content_size(&self) -> Result<u32, Error>;
}
