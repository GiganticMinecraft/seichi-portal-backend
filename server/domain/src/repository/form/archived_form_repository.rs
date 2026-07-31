use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::{
        answer::{AnswerId, ArchivedAnswerEntry},
        models::{ArchivedForm, ArchivedFormPagePosition, FormId},
    },
    pagination::{Page, PageRequest},
    types::authorization_guard::{Allowed, AuthorizationGuard, Create, Read, Update},
};

#[automock]
#[async_trait]
pub trait ArchivedFormRepository: Send + Sync + 'static {
    async fn list(
        &self,
        request: PageRequest<ArchivedFormPagePosition>,
        query: Option<String>,
    ) -> Result<Page<AuthorizationGuard<ArchivedForm, Read>, ArchivedFormPagePosition>, Error>;
    async fn get(
        &self,
        id: FormId,
    ) -> Result<Option<AuthorizationGuard<ArchivedForm, Read>>, Error>;
    /// 認可済みアーカイブフォーム配下の回答 Read proof を返します。
    async fn get_answer(
        &self,
        form: &Allowed<ArchivedForm, Read>,
        answer_id: AnswerId,
    ) -> Result<Option<Allowed<ArchivedAnswerEntry, Read>>, Error>;
    async fn archive(
        &self,
        form: Allowed<ArchivedForm, Create>,
    ) -> Result<AuthorizationGuard<ArchivedForm, Read>, Error>;
    async fn restore(&self, form: Allowed<ArchivedForm, Update>) -> Result<(), Error>;
}
