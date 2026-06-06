use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::{
        answer::models::{AnswerEntry, AnswerId},
        models::ActiveForm,
    },
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
    async fn list_by_form(
        &self,
        form: &Allowed<ActiveForm, Read>,
    ) -> Result<Vec<Allowed<AnswerEntry, Read>>, Error>;
    /// 渡された [`ActiveForm`] のガードを起点に、全フォームの回答を認可済みで返す。
    ///
    /// 「誰が読めるか」は引数で受け取った各 [`ActiveForm`] の読み取りガードが決めるため、
    /// 検索エンジンのインデックス構築のような全件取得でも認可を呼び出し側に明示させられる。
    /// 渡されなかったフォームの回答は結果に含まれない。
    async fn list_all(
        &self,
        forms: &[Allowed<ActiveForm, Read>],
    ) -> Result<Vec<Allowed<AnswerEntry, Read>>, Error>;
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
    async fn size(&self) -> Result<u32, Error>;
}
