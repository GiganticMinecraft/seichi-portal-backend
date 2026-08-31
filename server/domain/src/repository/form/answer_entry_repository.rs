use crate::account::models::UserId;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use errors::Error;
use mockall::automock;

use crate::{
    form::{
        answer::{
            AnswerEntry, AnswerId, AnswerLabelId, AnswerPagePosition, AnswerStatus,
            AnswerStatusChange, AnswerStatusHistoryEntry, AnswerStatusHistoryPagePosition,
            AnswerTitleHistoryEntry, AnswerTitleHistoryPagePosition,
        },
        models::{ActiveForm, FormId},
    },
    pagination::{Page, PageRequest},
    types::authorization_guard::{Allowed, Create, Read, Update},
};

/// 回答一覧に適用する絞り込み条件です。
///
/// `statuses` と `form_ids` の各要素は OR 条件、異なる種類の条件は AND 条件として扱い
/// ます。`label_ids` は指定されたすべてのラベルを持つ回答に絞り込みます。`form_ids` は
/// 横断一覧でだけ使われ、`None` はフォームによる制限なしを表します。
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct AnswerListFilter {
    statuses: Vec<AnswerStatus>,
    user_id: Option<UserId>,
    label_ids: Vec<AnswerLabelId>,
    created_after: Option<DateTime<Utc>>,
    created_before: Option<DateTime<Utc>>,
    form_ids: Option<Vec<FormId>>,
}

impl AnswerListFilter {
    pub fn new(
        statuses: Vec<AnswerStatus>,
        user_id: Option<UserId>,
        label_ids: Vec<AnswerLabelId>,
        created_after: Option<DateTime<Utc>>,
        created_before: Option<DateTime<Utc>>,
        form_ids: Option<Vec<FormId>>,
    ) -> Self {
        Self {
            statuses,
            user_id,
            label_ids,
            created_after,
            created_before,
            form_ids,
        }
    }

    pub fn statuses(&self) -> &[AnswerStatus] {
        &self.statuses
    }

    pub fn user_id(&self) -> Option<UserId> {
        self.user_id
    }

    pub fn label_ids(&self) -> &[AnswerLabelId] {
        &self.label_ids
    }

    pub fn created_after(&self) -> Option<DateTime<Utc>> {
        self.created_after
    }

    pub fn created_before(&self) -> Option<DateTime<Utc>> {
        self.created_before
    }

    pub fn form_ids(&self) -> Option<&[FormId]> {
        self.form_ids.as_deref()
    }

    /// 認可済みフォームだけを対象にするため、フォーム条件を置き換えます。
    pub fn restrict_to_form_ids(self, form_ids: Vec<FormId>) -> Self {
        Self {
            form_ids: Some(form_ids),
            ..self
        }
    }
}

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
        filter: AnswerListFilter,
    ) -> Result<Page<Allowed<AnswerEntry, Read>, AnswerPagePosition>, Error>;
    async fn list_all(
        &self,
        forms: &[Allowed<ActiveForm, Read>],
        request: PageRequest<AnswerPagePosition>,
        filter: AnswerListFilter,
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
    ) -> Result<Option<AnswerStatusChange>, Error>;
    async fn history(
        &self,
        answer: &Allowed<AnswerEntry, Read>,
        request: PageRequest<AnswerStatusHistoryPagePosition>,
    ) -> Result<Page<Allowed<AnswerStatusHistoryEntry, Read>, AnswerStatusHistoryPagePosition>, Error>;
    async fn title_history(
        &self,
        answer: &Allowed<AnswerEntry, Read>,
        request: PageRequest<AnswerTitleHistoryPagePosition>,
    ) -> Result<Page<Allowed<AnswerTitleHistoryEntry, Read>, AnswerTitleHistoryPagePosition>, Error>;
    /// 回答 (`answers`) の件数を返す。
    async fn size(&self) -> Result<u32, Error>;
    /// 回答本文 (`real_answers`) の件数を返す。
    async fn content_size(&self) -> Result<u32, Error>;
}
