use async_trait::async_trait;
use domain::form::models::{FormId, FormTitle};
use entities::form_meta_data;
use sea_orm::{ActiveModelTrait, ActiveValue, ActiveValue::Set};

use crate::database::{components::FormDatabase, connection::ConnectionPool};

#[async_trait]
impl FormDatabase for ConnectionPool {
    async fn create(&self, title: FormTitle) -> anyhow::Result<FormId> {
        let form_id = form_meta_data::ActiveModel {
            id: ActiveValue::NotSet,
            title: Set(title.title().to_owned()),
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
