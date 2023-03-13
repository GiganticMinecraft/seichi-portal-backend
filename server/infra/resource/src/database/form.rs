use async_trait::async_trait;
use domain::form::models::{FormId, FormName};
use entities::form_meta_data;
use sea_orm::{ActiveModelTrait, ActiveValue, ActiveValue::Set};

use crate::database::{components::FormDatabase, connection::ConnectionPool};

#[async_trait]
impl FormDatabase for ConnectionPool {
    async fn create(&self, name: FormName) -> anyhow::Result<FormId> {
        let form_id = form_meta_data::ActiveModel {
            id: ActiveValue::NotSet,
            name: Set(name.name().to_owned()),
            description: Set(None),
            created_at: Default::default(),
            updated_at: Default::default(),
        }
        .insert(&self.pool)
        .await?
        .id;

        Ok(form_id.into())
    }
}
