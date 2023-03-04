use crate::database::components::FormDatabase;
use crate::database::connection::ConnectionPool;
use async_trait::async_trait;
use domain::form::models::{FormId, FormName};
use entities::forms;
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ActiveValue};

#[async_trait]
impl FormDatabase for ConnectionPool {
    async fn create(&self, name: FormName) -> anyhow::Result<FormId> {
        let form_id = forms::ActiveModel {
            id: ActiveValue::NotSet,
            name: Set(name.name().to_owned()),
        }
        .insert(&self.pool)
        .await?
        .id;

        Ok(FormId(form_id))
    }
}
