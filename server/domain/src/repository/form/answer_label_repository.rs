use std::collections::HashMap;

use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::answer::{AnswerId, AnswerLabel, AnswerLabelId},
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Delete, Read, Update},
};

/// 回答 ID ごとの (読み取り認可前の) ラベル一覧
pub type AnswerLabelsByAnswerId = HashMap<AnswerId, Vec<AuthorizationGuard<AnswerLabel, Read>>>;

#[automock]
#[async_trait]
pub trait AnswerLabelRepository: Send + Sync + 'static {
    async fn create_label_for_answers(
        &self,
        label: Allowed<AnswerLabel, Create>,
    ) -> Result<(), Error>;
    async fn get_labels_for_answers(
        &self,
    ) -> Result<Vec<AuthorizationGuard<AnswerLabel, Read>>, Error>;
    async fn get_label_for_answers(
        &self,
        label_id: AnswerLabelId,
    ) -> Result<Option<AuthorizationGuard<AnswerLabel, Read>>, Error>;
    async fn get_labels_for_answers_by_label_ids(
        &self,
        label_ids: Vec<AnswerLabelId>,
    ) -> Result<Vec<AuthorizationGuard<AnswerLabel, Read>>, Error>;
    async fn get_labels_for_answers_by_answer_id(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<AuthorizationGuard<AnswerLabel, Read>>, Error>;
    /// 複数の回答に付いたラベルを 1 回の問い合わせでまとめて取得する。
    /// 回答一覧のように回答ごとに取得すると N+1 になる箇所で使う。
    /// ラベルが 1 つも付いていない回答はマップに含まれない。
    async fn get_labels_for_answers_by_answer_ids(
        &self,
        answer_ids: Vec<AnswerId>,
    ) -> Result<AnswerLabelsByAnswerId, Error>;
    async fn delete_label_for_answers(
        &self,
        label: Allowed<AnswerLabel, Delete>,
    ) -> Result<(), Error>;
    async fn edit_label_for_answers(
        &self,
        label: Allowed<AnswerLabel, Update>,
    ) -> Result<(), Error>;
    async fn replace_answer_labels(
        &self,
        answer_id: AnswerId,
        labels: Vec<Allowed<AnswerLabel, Update>>,
    ) -> Result<(), Error>;
    async fn size(&self) -> Result<u32, Error>;
}
