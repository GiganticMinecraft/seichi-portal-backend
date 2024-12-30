use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::models::{FormId, FormLabel, FormLabelId},
    types::authorization_guard::{AuthorizationGuard, Create, Delete, Read, Update},
    user::models::User,
};

#[automock]
#[async_trait]
pub trait FormLabelRepository: Send + Sync + 'static {
    async fn create_label_for_forms(
        &self,
        label: AuthorizationGuard<FormLabel, Create>,
        actor: &User,
    ) -> Result<(), Error>;
    async fn fetch_labels(&self) -> Result<Vec<AuthorizationGuard<FormLabel, Read>>, Error>;
    async fn fetch_label(
        &self,
        id: FormLabelId,
    ) -> Result<Option<AuthorizationGuard<FormLabel, Read>>, Error>;
    async fn delete_label_for_forms(
        &self,
        label: AuthorizationGuard<FormLabel, Delete>,
        actor: &User,
    ) -> Result<(), Error>;
    async fn edit_label_for_forms(
        &self,
        id: FormLabelId,
        label: AuthorizationGuard<FormLabel, Update>,
        actor: &User,
    ) -> Result<(), Error>;
    // TODO: replace_form_labelsはFormRepositoryに移動する
    async fn replace_form_labels(
        &self,
        form_id: FormId,
        label_ids: Vec<FormLabelId>,
    ) -> Result<(), Error>;
}
