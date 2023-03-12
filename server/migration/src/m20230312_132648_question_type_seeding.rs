use entities::question_types;
use sea_orm_migration::prelude::*;

use crate::sea_orm::{ActiveValue, ActiveValue::Set, EntityTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();

        let models = vec!["TEXT", "CHECKBOX", "PULLDOWN"]
            .into_iter()
            .map(|question_type| question_types::ActiveModel {
                id: ActiveValue::NotSet,
                question_type: Set(question_type.to_owned()),
            })
            .collect::<Vec<question_types::ActiveModel>>();

        question_types::Entity::insert_many(models)
            .exec(connection)
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        question_types::Entity::delete_many()
            .exec(manager.get_connection())
            .await?;

        Ok(())
    }
}
