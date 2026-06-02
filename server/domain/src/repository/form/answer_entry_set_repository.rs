use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::{
        answer::models::AnswerEntry, answer_entry_set::models::AnswerEntrySet, models::ActiveForm,
    },
    types::authorization_guard::{Allowed, Create, Read, Update},
};

/// フォームに紐づく回答 ([`AnswerEntry`]) の集合 ([`AnswerEntrySet`]) を永続化するリポジトリ。
///
/// 回答にまつわるポリシー（公開範囲・受付期間など）は [`crate::form::models::ActiveForm`] が
/// 保持するため、通常の取得は認可済みの [`ActiveForm`] を起点にし、フォームへの所属検証を
/// 通過した [`Allowed<AnswerEntrySet, _>`] だけを返す。
///
/// [`ActiveForm`]: crate::form::models::ActiveForm
#[automock]
#[async_trait]
pub trait AnswerEntrySetRepository: Send + Sync + 'static {
    async fn get_read(
        &self,
        form: &Allowed<ActiveForm, Read>,
    ) -> Result<Option<Allowed<AnswerEntrySet, Read>>, Error>;
    async fn get_update(
        &self,
        form: &Allowed<ActiveForm, Update>,
    ) -> Result<Option<Allowed<AnswerEntrySet, Update>>, Error>;
    async fn list_read_by_forms(
        &self,
        forms: &[Allowed<ActiveForm, Read>],
    ) -> Result<Vec<Allowed<AnswerEntrySet, Read>>, Error>;
    async fn add_entry(
        &self,
        answer_entry_set: &Allowed<AnswerEntrySet, Read>,
        answer_entry: &Allowed<AnswerEntry, Create>,
    ) -> Result<(), Error>;
    async fn update_entry(
        &self,
        answer_entry_set: &Allowed<AnswerEntrySet, Update>,
        answer_entry: &Allowed<AnswerEntry, Update>,
    ) -> Result<(), Error>;
    async fn size_entries(&self) -> Result<u32, Error>;
}
