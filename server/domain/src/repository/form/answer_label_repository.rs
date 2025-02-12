use async_trait::async_trait;
use errors::Error;
use mockall::automock;

use crate::{
    form::answer::models::{AnswerId, AnswerLabel, AnswerLabelId},
    types::{
        authorization_guard::AuthorizationGuard,
        authorization_guard_with_context::{Create, Delete, Read, Update},
    },
    user::models::User,
};

#[automock]
#[async_trait]
pub trait AnswerLabelRepository: Send + Sync + 'static {
    async fn create_label_for_answers(
        &self,
        actor: &User,
        label: AuthorizationGuard<AnswerLabel, Create>,
    ) -> Result<(), Error>;
    async fn get_labels_for_answers(
        &self,
    ) -> Result<Vec<AuthorizationGuard<AnswerLabel, Read>>, Error>;
    async fn get_label_for_answers(
        &self,
        label_id: AnswerLabelId,
    ) -> Result<Option<AuthorizationGuard<AnswerLabel, Read>>, Error>;
    async fn get_labels_for_answers_by_answer_id(
        &self,
        answer_id: AnswerId,
    ) -> Result<Vec<AuthorizationGuard<AnswerLabel, Read>>, Error>;
    async fn delete_label_for_answers(
        &self,
        actor: &User,
        label: AuthorizationGuard<AnswerLabel, Delete>,
    ) -> Result<(), Error>;
    async fn edit_label_for_answers(
        &self,
        actor: &User,
        label: AuthorizationGuard<AnswerLabel, Update>,
    ) -> Result<(), Error>;
    async fn replace_answer_labels(
        &self,
        answer_id: AnswerId,
        label_ids: Vec<AnswerLabelId>,
    ) -> Result<(), Error>;
}
